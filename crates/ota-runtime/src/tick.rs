//! `Engine::run_day`: the §6 steps in order.

use crate::state::take_human_inputs;
use crate::{Engine, EventCtx};
use anyhow::Result;
use chrono::Datelike;
use ota_core::*;
use ota_decide::eval_bool;
use ota_llm::LlmRequest;
use ota_tools::governance;
use ota_tools::history::{DayAgg, DevTrack};
use ota_tools::lattice::{self, MetricCtx};
use std::collections::{BTreeMap, BTreeSet};

/// Everything produced while running one day.
pub(crate) struct Today {
    pub day: Day,
    pub monday: bool,
    pub month_start: bool,
    pub quarter_start: bool,
    pub rec: DayRecord,
    pub proposals: Vec<Decision>,
    /// Decisions approved today (any day of origin) to execute in step 9.
    pub to_execute: Vec<Id>,
    /// Earlier decisions changed today.
    pub touched: BTreeSet<Id>,
    pub agg: DayAgg,
    /// module → deviation ids routed today
    pub routed: BTreeMap<NodeId, Vec<Id>>,
    /// module → own decisions sealed today
    pub sealed: BTreeMap<NodeId, Vec<Id>>,
    /// module → assumption ids flagged today
    pub assumption_flags: BTreeMap<NodeId, Vec<Id>>,
    pub okr_flags: Vec<Id>,
    /// telemetry signal id by metric (for decision `information`)
    pub signal_of_metric: BTreeMap<String, Id>,
}

impl Engine {
    pub fn weekday_index(spec: &ota_spec::CompanySpec) -> u32 {
        let (y, m, d) = ota_spec::loader::chrono_like_date(&spec.company.start_date).unwrap_or((2025, 1, 1));
        chrono::NaiveDate::from_ymd_opt(y, m, d).map(|x| x.weekday().num_days_from_monday()).unwrap_or(0)
    }

    pub(crate) fn enabled(&self, tool: &str, day: Day) -> bool {
        self.spec.tool_enabled(tool, day)
    }

    /// Accountable human for a node (module → its accountable; person → itself).
    pub(crate) fn human_for(&self, node: &str) -> NodeId {
        if self.spec.is_human(node) {
            return node.to_string();
        }
        self.spec.module(node).map(|m| m.accountable.clone())
            .or_else(|| self.spec.leaders().next().map(|p| p.id.clone()))
            .unwrap_or_default()
    }

    pub(crate) fn first_leader(&self) -> NodeId {
        self.spec.leaders().next().map(|p| p.id.clone()).unwrap_or_default()
    }

    /// Run one simulated day and persist its record.
    pub fn run_day(&mut self) -> Result<DayRecord> {
        let t0 = std::time::Instant::now();
        let day = self.st.day + 1;
        let date = self.date_of(day);
        self.st.rt.ids.set_day(day);
        self.llm.begin_day(day);
        self.llm.force_scripted = day <= self.opts.scripted_until;
        self.st.rt.bus.begin_day();
        let wd = date.weekday().num_days_from_monday();
        let month_start = date.day() == 1;
        let quarter_start = month_start && matches!(date.month(), 1 | 4 | 7 | 10);
        let mut cadences = vec![Cadence::Daily];
        if wd == 0 { cadences.push(Cadence::Weekly); }
        if month_start { cadences.push(Cadence::Monthly); }
        if quarter_start { cadences.push(Cadence::Quarterly); }
        let tools_enabled: Vec<String> = self.spec.toolkit.iter().filter(|t| t.enabled_from <= day).map(|t| t.id.clone()).collect();
        let mut td = Today {
            day, monday: wd == 0, month_start, quarter_start,
            rec: DayRecord {
                day, date: date.format("%Y-%m-%d").to_string(), weekday: WEEKDAYS[wd as usize].into(),
                cadences, tools_enabled, llm_mode: self.llm.mode_label().into(), ..Default::default()
            },
            proposals: vec![], to_execute: vec![], touched: BTreeSet::new(), agg: DayAgg { day, ..Default::default() },
            routed: BTreeMap::new(), sealed: BTreeMap::new(), assumption_flags: BTreeMap::new(), okr_flags: vec![],
            signal_of_metric: BTreeMap::new(),
        };

        // Real-human inputs first (they pre-empt stand-ins).
        for h in take_human_inputs(&self.store) {
            self.apply_human_input(&mut td, h);
        }

        self.step_environment(&mut td)?;
        self.step_lattice(&mut td);
        self.step_assumptions(&mut td);
        self.step_switches(&mut td);
        self.step_think(&mut td);
        self.step_governance(&mut td);
        self.step_humans(&mut td);
        self.step_act(&mut td)?;
        if td.monday { self.step_weekly(&mut td); }
        self.step_daily_monitors(&mut td);
        if td.month_start { self.step_monthly(&mut td); }
        if td.quarter_start || day == 1 { self.step_quarterly(&mut td); }
        self.step_autonomy_reviews(&mut td);

        // close the day
        td.rec.messages = std::mem::take(&mut self.st.rt.bus.today);
        let ids: Vec<Id> = td.touched.iter().cloned().collect();
        let new_ids: BTreeSet<Id> = td.rec.decisions.iter().map(|d| d.id.clone()).collect();
        for d in td.rec.decisions.iter_mut() {
            if let Some(x) = self.st.ledger.get(&d.id) { *d = x.clone(); }
        }
        td.rec.decision_updates = ids.iter().filter(|i| !new_ids.contains(*i)).filter_map(|i| self.st.ledger.get(i).cloned()).collect();
        for p in &self.spec.people {
            td.rec.queue_open.insert(p.id.clone(), self.st.rt.queue.iter().filter(|q| q.person == p.id).count());
        }
        td.rec.llm_calls = self.llm.drain_calls();
        self.st.rt.findings.extend(td.rec.findings.iter().cloned());
        let n = self.st.rt.findings.len();
        if n > 300 { self.st.rt.findings.drain(..n - 300); }
        self.st.rt.challenges.retain(|_, c| c.day + 90 > day);
        self.compute_metrics(&mut td);
        let mut agg = std::mem::take(&mut td.agg);
        agg.decisions = td.rec.decisions.len();
        self.st.history.push(agg);
        self.st.history.compact(day);
        self.st.calibration.compact(day, 400);
        td.rec.log.push(format!("day {day} done in {} ms", t0.elapsed().as_millis()));

        // persist
        self.store.write_day(&td.rec)?;
        let mut kp = serde_json::Map::new();
        kp.insert("day".into(), day.into());
        kp.insert("date".into(), td.rec.date.clone().into());
        for (k, v) in &td.rec.kpis { kp.insert(k.clone(), (*v).into()); }
        for (k, v) in metric_series(&td.rec.metrics) { kp.insert(k, v.into()); }
        self.store.append("kpis", &[serde_json::Value::Object(kp)])?;
        let ledger_events: Vec<serde_json::Value> = td.rec.decisions.iter().chain(td.rec.decision_updates.iter())
            .map(|d| serde_json::json!({"day": day, "id": d.id, "class": d.class, "module": d.module, "status": d.status, "tier": d.tier,
                                        "what": d.what, "success": d.outcome.as_ref().and_then(|o| o.success)})).collect();
        self.store.append("ledger", &ledger_events)?;
        let calls: Vec<serde_json::Value> = td.rec.llm_calls.iter().map(|c| {
            let mut v = serde_json::to_value(c).unwrap_or_default();
            v["day"] = day.into();
            v
        }).collect();
        self.store.append("llm", &calls)?;
        let human: Vec<&HumanAction> = td.rec.human_actions.iter().collect();
        self.store.append("human", &human)?;
        self.st.day = day;
        Ok(td.rec)
    }

    // ------------------------------------------------------------------ 1–2 environment + ingest

    fn step_environment(&mut self, td: &mut Today) -> Result<()> {
        let day = td.day;
        let recent_events = self.st.rt.recent_events.clone();
        let recent_kpis = self.st.rt.recent_kpis.clone();
        let mut ctx = EventCtx { day, date: td.rec.date.clone(), spec: &self.spec, recent_events: &recent_events,
                                 recent_kpis: &recent_kpis, llm: &mut self.llm };
        let mut events = self.world.generate_events(day, &mut ctx);
        for e in events.iter_mut() {
            e.day = day;
            if e.id.is_empty() { e.id = self.st.rt.ids.next("E"); }
        }
        let executed = std::mem::take(&mut self.st.rt.pending_actions);
        let wd = self.world.advance(day, &events, &executed)?;
        for e in &events {
            self.st.rt.last_seen.insert(e.kind.clone(), day);
            for t in &e.tags { self.st.rt.last_seen.insert(t.clone(), day); }
        }
        self.st.rt.recent_events.extend(events.iter().cloned());
        self.st.rt.recent_events.retain(|e| e.day + 14 > day);
        self.st.rt.research.extend(wd.research.iter().cloned());
        self.st.rt.research.retain(|e| e.day + 35 > day);

        // ingest: telemetry → internal signals, events → external signals
        for (k, v) in &wd.kpis {
            let id = self.st.rt.ids.next("S");
            td.signal_of_metric.insert(k.clone(), id.clone());
            td.rec.signals.push(Signal { id, day, source: "telemetry".into(), kind: "kpi".into(), metric: Some(k.clone()),
                                         value: Some(*v), text: String::new(), provenance: Provenance::Internal, tags: vec![] });
        }
        for e in &events {
            td.rec.signals.push(Signal { id: self.st.rt.ids.next("S"), day, source: format!("event:{}", e.id), kind: e.kind.clone(),
                                         metric: None, value: None, text: e.title.clone(), provenance: Provenance::External, tags: e.tags.clone() });
        }
        for mut s in wd.signals.clone() {
            s.day = day;
            if s.id.is_empty() { s.id = self.st.rt.ids.next("S"); }
            td.rec.signals.push(s);
        }
        // external share of non-telemetry inputs (echo chamber); telemetry is the baseline stream
        let non_tel: Vec<&Signal> = td.rec.signals.iter().filter(|s| s.source != "telemetry").collect();
        td.agg.external_share = (!non_tel.is_empty()).then(|| non_tel.iter().filter(|s| s.provenance == Provenance::External).count() as f64 / non_tel.len() as f64);
        td.agg.untraced_human_actions = wd.untraced_human_actions;
        self.st.lattice.record(day, &wd.kpis);
        self.st.rt.recent_kpis = wd.kpis.clone();
        td.rec.kpis = wd.kpis;
        td.rec.events = events;
        td.rec.log.extend(wd.notes);

        // anomalies are journaled from day 1 (data estate); clustering/promotion needs the tool
        let anomalies = wd.anomalies;
        td.agg.bad_news_by_humans += anomalies.iter().filter(|a| self.spec.is_human(&a.by)).count();
        if self.enabled("anomalies", day) {
            let (added, devs) = self.st.anomalies.add(day, anomalies, &mut self.st.rt.ids);
            td.rec.anomalies = added;
            td.rec.deviations.extend(devs);
        } else {
            for mut a in anomalies {
                if a.id.is_empty() { a.id = self.st.rt.ids.next("AN"); }
                td.rec.anomalies.push(a);
            }
        }
        Ok(())
    }

    // ------------------------------------------------------------------ 3 lattice

    fn step_lattice(&mut self, td: &mut Today) {
        let day = td.day;
        if !self.enabled("lattice", day) {
            return;
        }
        // L1 coverage
        let mut expected: Vec<String> = self.spec.modules.iter().flat_map(|m| m.inputs.iter().cloned()).collect();
        expected.extend(self.spec.kpis.iter().map(|k| k.metric.clone()));
        for m in self.st.lattice.coverage_gaps(&expected, &td.rec.kpis) {
            // only report metrics that have ever existed (or never: instrument gap) once a week
            if !td.monday { continue; }
            td.rec.deviations.push(Deviation { id: self.st.rt.ids.next("DV"), day, metric: m, value: 0.0, baseline: 0.0, sigma: 0.0, z: 0.0,
                level: 1, detector: "coverage_gap".into(), interpretation: Some("expected metric not reported (L1 coverage)".into()),
                routed_to: vec![], suppressed_by_budget: false, disposition: None });
        }
        // L2/L3 detection
        let devs = self.st.lattice.detect(day, &td.rec.kpis, &mut self.st.rt.ids);
        td.rec.deviations.extend(devs);

        // L4 interpretation: one batched small-tier call for the top flags
        let top: Vec<&Deviation> = td.rec.deviations.iter().filter(|d| d.level >= 3 && d.interpretation.is_none()).take(5).collect();
        if !top.is_empty() {
            let items: Vec<serde_json::Value> = top.iter().map(|d| serde_json::json!({"deviation": d.id, "metric": d.metric, "value": d.value,
                "baseline": d.baseline, "z": d.z, "detector": d.detector})).collect();
            let events: Vec<&str> = td.rec.events.iter().map(|e| e.title.as_str()).collect();
            let req = LlmRequest {
                tier: "small".into(), purpose: "lattice_interpret".into(), module: Some("reality-lattice".into()),
                system: "You are the Reality Lattice interpretation layer (L4). For each flagged deviation give a one-sentence, falsifiable interpretation. Separate fact from inference.".into(),
                user: format!("Day {day}. Flags: {}\nToday's external events: {:?}", serde_json::Value::Array(items.clone()), events),
                schema_name: "interpretations".into(), schema: Some(governance::interpret_schema()),
                context: serde_json::json!({"day": day, "deviations": items, "events": events}), day,
            };
            let r = self.llm.call(&req);
            if let Some(v) = r.value {
                for it in v["interpretations"].as_array().cloned().unwrap_or_default() {
                    if let (Some(id), Some(text)) = (it["deviation"].as_str(), it["interpretation"].as_str()) {
                        if let Some(d) = td.rec.deviations.iter_mut().find(|d| d.id == id) {
                            d.interpretation = Some(text.to_string());
                            d.level = d.level.max(4);
                        }
                    }
                }
            }
        }

        // L5 routing with alert budgets
        let spec = &self.spec;
        let recipients = |metric: &str| -> Vec<NodeId> {
            if metric.starts_with("anomaly:") {
                // the shadow group reports directly to leadership (§13.10)
                return spec.leaders().map(|p| p.id.clone()).collect();
            }
            let mut v: Vec<NodeId> = spec.modules.iter().filter(|m| m.inputs.iter().any(|i| i == metric)).map(|m| m.id.clone()).collect();
            if let Some(o) = spec.kpi(metric).and_then(|k| k.owner.clone()) {
                if !v.contains(&o) && spec.module(&o).is_some() { v.push(o); }
            }
            v
        };
        let budget = |node: &str| spec.module(node).and_then(|m| m.alert_budget).unwrap_or(spec.company.alert_budget);
        let routable: Vec<usize> = (0..td.rec.deviations.len()).filter(|i| td.rec.deviations[*i].level >= 3).collect();
        let mut devs: Vec<Deviation> = routable.iter().map(|i| td.rec.deviations[*i].clone()).collect();
        let queued = lattice::route(&mut devs, &recipients, &budget);
        for (i, d) in routable.into_iter().zip(devs) {
            td.rec.deviations[i] = d;
        }
        for d in &td.rec.deviations {
            for r in &d.routed_to {
                td.routed.entry(r.clone()).or_default().push(d.id.clone());
            }
            td.agg.alerts_routed += d.routed_to.len();
            if d.suppressed_by_budget { td.agg.alerts_suppressed += 1; }
            if d.z.abs() >= 2.0 || d.detector == "anomaly_cluster" {
                self.st.history.devs.push(DevTrack { id: d.id.clone(), day, metric: d.metric.clone(), z: d.z,
                                                     routed: !d.routed_to.is_empty(), acted_day: None, dismissed_day: None });
            }
        }
        for (node, ids) in queued {
            for id in &ids {
                if let Some(d) = td.rec.deviations.iter().find(|d| &d.id == id) {
                    self.st.rt.open_devs.insert(id.clone(), d.clone());
                }
            }
            self.st.rt.alert_queue.entry(node).or_default().extend(ids);
        }
        td.agg.alerts_open = self.st.rt.alert_queue.values().map(|v| v.len()).sum();
        // leaders receive anomaly clusters as messages (bad news, no suppression)
        let anomaly_devs: Vec<Deviation> = td.rec.deviations.iter().filter(|d| d.detector == "anomaly_cluster").cloned().collect();
        for d in anomaly_devs {
            let to: Vec<NodeId> = self.spec.leaders().map(|p| p.id.clone()).collect();
            self.st.rt.bus.send(&mut self.st.rt.ids, day, "reality-lattice", to, Intent::Inform, format!("Anomaly cluster: {}", d.metric),
                                d.interpretation.clone().unwrap_or_default(), vec![d.id.clone()], 1);
        }
        // scenario proposals from large surprises
        if self.enabled("scenarios", day) {
            let big: Vec<Deviation> = td.rec.deviations.iter().filter(|d| d.z.abs() >= 4.0 || d.detector == "anomaly_cluster").cloned().collect();
            for d in big {
                if self.st.rt.scenario_proposed.get(&d.metric).map(|x| x + 30 > day).unwrap_or(false) { continue; }
                self.st.rt.scenario_proposed.insert(d.metric.clone(), day);
                let tier = if d.z.abs() >= 6.0 { ScenarioTier::Urgent } else if d.detector == "anomaly_cluster" { ScenarioTier::Major } else { ScenarioTier::Minor };
                let dir = if d.z >= 0.0 { "up" } else { "down" };
                let node = ScenarioNode {
                    id: format!("S-{}", self.st.rt.ids.next("SC")), parent: vec![], title: format!("Regime shift: {} {dir} ({:+.1}σ)", d.metric, d.z),
                    p: 0.2, leading_indicators: if d.detector == "z" { vec![IndicatorRule { expr: format!("abs(z({})) > 2", d.metric), effect: "confirm".into() }] } else { vec![] },
                    assumptions: vec![], tier, status: String::new(), approved_by: None, proposed_by: Some("scenario-tree".into()), created: day, history: vec![],
                };
                let n = self.st.scenarios.propose(node, day);
                if tier == ScenarioTier::Urgent {
                    let leader = self.first_leader();
                    self.enqueue(day, &leader, QueueKind::ScenarioApproval, None, format!("Urgent scenario: {}", n.title),
                                 serde_json::json!({"scenario": n.id}), true);
                }
                td.rec.scenario_changes.push(n);
            }
        }
    }

    // ------------------------------------------------------------------ 4 assumptions + OKRs

    fn step_assumptions(&mut self, td: &mut Today) {
        let day = td.day;
        if !self.enabled("assumptions", day) {
            return;
        }
        let kpis = td.rec.kpis.clone();
        let updates = {
            let ctx = MetricCtx::new(day, &kpis, &self.st.lattice, &self.st.rt.last_seen);
            self.st.assumptions.evaluate(day, &ctx)
        };
        for u in &updates {
            let Some(a) = self.st.assumptions.get(&u.assumption).cloned() else { continue };
            let owner_module = a.module.clone().unwrap_or_else(|| a.owner.clone());
            td.assumption_flags.entry(owner_module.clone()).or_default().push(a.id.clone());
            match u.status {
                AssumptionStatus::Refuted => {
                    // bad-news rule: straight to the accountable human, no suppression
                    let to = vec![self.human_for(&a.owner)];
                    self.st.rt.bus.send(&mut self.st.rt.ids, day, "assumption-cycle", to, Intent::Inform,
                                        format!("Assumption refuted: {}", a.id), format!("{} — {}", a.text, u.reason), vec![a.id.clone()], 1);
                }
                AssumptionStatus::Stale if a.load_bearing => {
                    let who = self.human_for(&a.owner);
                    self.enqueue(day, &who, QueueKind::Review, None, format!("Archive or renew stale assumption {}", a.id),
                                 serde_json::json!({"assumption": a.id, "text": a.text, "last_confirmed": a.last_confirmed}), false);
                }
                _ => {}
            }
        }
        td.agg.stale_assumptions = self.st.assumptions.graveyard().len();
        td.rec.assumption_updates = updates;

        if self.enabled("okrs", day) {
            let flagged = ota_tools::okrs::update(&mut self.st.rt.okrs, &kpis, &self.st.assumptions, day);
            for o in &flagged {
                td.okr_flags.push(o.id.clone());
                self.st.rt.bus.send(&mut self.st.rt.ids, day, "living-okrs", vec![o.setter.clone()], Intent::Inform,
                                    format!("OKR flagged: {}", o.id), o.flagged.clone().unwrap_or_default(), vec![o.id.clone()], 1);
            }
            td.rec.okr_flags = flagged;
        }
    }

    // ------------------------------------------------------------------ 5 switches + forecasts

    fn step_switches(&mut self, td: &mut Today) {
        let day = td.day;
        let kpis = td.rec.kpis.clone();
        let due = self.st.ledger.due(day);
        td.agg.switches_due = due.len();
        for id in due {
            let on_time = self.st.ledger.get(&id).map(|d| d.switches.check_by + 3 >= day).unwrap_or(false);
            let outcome = {
                let ctx = MetricCtx::new(day, &kpis, &self.st.lattice, &self.st.rt.last_seen);
                self.st.ledger.seal(&id, day, &ctx)
            };
            if on_time { td.agg.switches_sealed_on_time += 1; }
            td.touched.insert(id.clone());
            let Some(o) = outcome else { continue };
            let d = self.st.ledger.get(&id).cloned().unwrap();
            td.sealed.entry(d.module.clone()).or_default().push(id.clone());
            if o.success == Some(false) {
                td.agg.bad_news_by_humans += 0; // AI-raised; counted in metrics as bad news
                let to = vec![d.accountable.clone()];
                self.st.rt.bus.send(&mut self.st.rt.ids, day, &d.module, to, Intent::Report, format!("Outcome sealed: {} failed", d.id),
                                    o.actual.clone(), vec![d.id.clone()], 1);
                // stop signal crossed on a reversible decision → propose a stop (revert)
                if o.stop_hit && d.reversibility != Reversibility::Irreversible {
                    let mut stop = d.clone();
                    stop.id = self.st.rt.ids.next("D");
                    stop.what = format!("Stop {}: revert `{}`", d.id, d.what);
                    stop.when = day;
                    stop.action = ActionSpec { kind: "revert".into(), params: serde_json::json!({"decision": d.id, "action": d.action}) };
                    stop.information = vec![d.id.clone()];
                    stop.expected = "metrics return to pre-decision baseline".into();
                    stop.switches = DecisionSwitch { confirm_if: d.switches.confirm_if.clone(), stop_if: "false".into(), check_by: day + 7 };
                    stop.notes = vec![format!("stops {}", d.id)];
                    stop.reasoning = format!("Stop signal `{}` crossed; the decision was a hypothesis and the exit fires.", d.switches.stop_if);
                    stop.forecasts = vec![]; stop.approvals = vec![]; stop.overrides = vec![]; stop.challenges = vec![];
                    stop.premortem = None; stop.gate = None; stop.outcome = None; stop.actions = vec![];
                    stop.status = DecisionStatus::Proposed; stop.tier = Tier::Rule; stop.kind = DecisionKind::Routine;
                    stop.stakes_eur = d.stakes_eur * 0.2; stop.novelty = 0.1; stop.confidence = 0.7;
                    stop.premortem_started_ts = None; stop.committed_ts = None; stop.committed_day = None;
                    td.proposals.push(stop);
                }
            }
        }
        // overdue: sealed late or still open past check_by + 3
        td.agg.switches_overdue = self.st.ledger.decisions.values()
            .filter(|d| d.outcome.is_none() && d.switches.check_by + 3 < day && matches!(d.status, DecisionStatus::Executing | DecisionStatus::Approved)).count();

        // forecasts
        for idx in self.st.calibration.due(day) {
            let f = self.st.calibration.forecasts[idx].clone();
            let outcome = match (&f.resolve_if, f.decision.as_ref().and_then(|d| self.st.ledger.get(d))) {
                (Some(e), _) => {
                    let ctx = MetricCtx::new(day, &kpis, &self.st.lattice, &self.st.rt.last_seen);
                    match eval_bool(e, &ctx) { Ok(b) => Some(b), Err(_) => None }
                }
                (None, Some(d)) => match &d.outcome {
                    Some(o) => Some(o.success == Some(true)),
                    None if f.resolves_on + 14 < day => Some(false),
                    None => None,
                },
                (None, None) => Some(false),
            };
            let outcome = outcome.or_else(|| (f.resolves_on + 14 < day).then_some(false));
            if let Some(o) = outcome {
                td.rec.forecasts_resolved.push(self.st.calibration.resolve(idx, o));
            }
        }
    }

    pub(crate) fn enqueue(&mut self, day: Day, person: &str, kind: QueueKind, decision: Option<Id>, subject: String, payload: serde_json::Value, blocking: bool) -> Id {
        let id = self.st.rt.ids.next("Q");
        self.st.rt.queue.push(QueueItem { id: id.clone(), person: person.to_string(), kind, created_day: day, decision, subject, payload, blocking });
        id
    }

    // ------------------------------------------------------------------ 9 act

    fn step_act(&mut self, td: &mut Today) -> Result<()> {
        let day = td.day;
        let ids = std::mem::take(&mut td.to_execute);
        for id in ids {
            let Some(d) = self.st.ledger.get(&id).cloned() else { continue };
            if d.status != DecisionStatus::Approved { continue; }
            if d.action.is_hold() {
                if let Some(x) = self.st.ledger.get_mut(&id) { x.status = DecisionStatus::Executing; }
                continue;
            }
            let mut a = Action { id: self.st.rt.ids.next("A"), decision: d.id.clone(), module: d.module.clone(), kind: d.action.kind.clone(),
                                 params: d.action.params.clone(), executed_day: day, effect_summary: String::new() };
            a.effect_summary = match self.world.apply_action(day, &a) {
                Ok(s) => s,
                Err(e) => format!("action failed: {e}"),
            };
            if d.action.kind == "revert" {
                if let Some(orig) = d.action.params["decision"].as_str() {
                    if let Some(o) = self.st.ledger.get_mut(orig) {
                        o.status = DecisionStatus::Reverted;
                        td.touched.insert(orig.to_string());
                    }
                }
            }
            self.st.rt.last_seen.insert(d.class.clone(), day);
            self.st.rt.last_seen.insert(a.kind.clone(), day);
            if let Some(x) = self.st.ledger.get_mut(&id) {
                x.status = DecisionStatus::Executing;
                x.actions.push(a.id.clone());
            }
            if d.when != day { td.touched.insert(id.clone()); }
            self.st.rt.pending_actions.push(a.clone());
            td.rec.actions.push(a);
        }
        Ok(())
    }
}

fn metric_series(m: &DayMetrics) -> Vec<(String, f64)> {
    let mut v = vec![
        ("ota.decisions".to_string(), m.decisions_committed as f64),
        ("ota.throughput".to_string(), m.throughput_per_day),
        ("ota.human_minutes".to_string(), m.human_minutes),
        ("ota.alerts_routed".to_string(), m.alerts_routed as f64),
        ("ota.alerts_suppressed".to_string(), m.alerts_suppressed as f64),
        ("ota.llm_cost_usd".to_string(), m.llm_cost_usd),
    ];
    let opt = [("ota.decision_latency_days", m.decision_latency_days), ("ota.loop_speed_days", m.loop_speed_days),
               ("ota.learning_rate", m.learning_rate), ("ota.hops_to_decider", m.hops_to_decider),
               ("ota.real_human_share", m.real_human_share), ("ota.external_signal_share", m.external_signal_share),
               ("ota.ledger_completeness", m.ledger_completeness), ("ota.brier_30d", m.brier_30d)];
    for (k, x) in opt {
        if let Some(x) = x { v.push((k.to_string(), x)); }
    }
    v
}
