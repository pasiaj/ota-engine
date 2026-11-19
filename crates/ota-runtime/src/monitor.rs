//! Steps 10–13 (weekly, daily monitors, monthly, quarterly), autonomy reviews, daily
//! metrics, toolkit-module traces and the public state view.

use crate::tick::Today;
use crate::Engine;
use chrono::Datelike;
use ota_core::*;
use ota_llm::LlmRequest;
use ota_tools::calibration::profile;
use ota_tools::governance;
use ota_tools::lattice::MetricCtx;
use ota_tools::learning;
use ota_tools::sentinel::Input as SentinelInput;
use serde_json::{json, Value};
use std::collections::BTreeMap;

impl Engine {
    fn autonomy_evidence(&self, class: &str, day: Day) -> AutonomyEvidence {
        let from = day.saturating_sub(90);
        let p = profile(self.st.calibration.forecasts.iter().filter(|f| !f.human && f.topic == class && f.resolves_on >= from));
        let (mut agree, mut total) = (0usize, 0usize);
        for d in self.st.ledger.decisions.values().filter(|d| d.class == class && d.when >= from) {
            for a in &d.approvals {
                match a.verdict {
                    HumanVerdict::Approve => { agree += 1; total += 1; }
                    HumanVerdict::Reject | HumanVerdict::Amend => total += 1,
                    HumanVerdict::Defer => {}
                }
            }
        }
        AutonomyEvidence { brier: p.brier, n: p.n, agreement_rate: (total > 0).then(|| agree as f64 / total as f64), threshold: 0.0 }
    }

    fn propose_autonomy(&mut self, td: &mut Today, scheduled: Option<&str>) {
        let day = td.day;
        let classes: Vec<ota_spec::DecisionClass> = self.spec.classes.values().cloned().collect();
        for c in classes {
            let sched = match scheduled {
                Some(_) => match self.st.autonomy.review_due(&c, td.month_start, td.quarter_start) { Some(s) => Some(s), None => continue },
                None => None,
            };
            let ev = self.autonomy_evidence(&c.id, day);
            if let Some(ch) = self.st.autonomy.evaluate(&c, day, ev, sched, &mut self.st.rt.ids) {
                self.enqueue(day, &c.accountable, QueueKind::AutonomyChange, None,
                             format!("Autonomy {}: {} → {} ({})", c.id, ch.from, ch.to, ch.trigger), json!({"change": ch.id, "class": c.id}), false);
                td.rec.autonomy_changes.push(ch);
            }
        }
    }

    pub(crate) fn step_autonomy_reviews(&mut self, td: &mut Today) {
        if self.enabled("autonomy", td.day) && (td.month_start || td.quarter_start) {
            self.propose_autonomy(td, Some("scheduled"));
        }
    }

    // ------------------------------------------------------------------ 10 weekly

    pub(crate) fn step_weekly(&mut self, td: &mut Today) {
        let day = td.day;
        if self.enabled("calibration", day) {
            let mut pubm = BTreeMap::new();
            for m in &self.spec.modules {
                pubm.insert(m.id.clone(), profile(self.st.calibration.forecasts.iter().filter(|f| !f.human && f.by == m.id)).brier);
            }
            self.st.rt.published_module_brier = pubm;
        }
        if self.enabled("autonomy", day) {
            self.propose_autonomy(td, None); // off-cycle demotions when thresholds are crossed
        }
        if self.enabled("double_loop", day) {
            let f = self.st.double_loop.weekly(&self.st.ledger, day, &mut self.st.rt.ids);
            for x in &f {
                let class = x.text.split(':').next().unwrap_or("").to_string();
                let to = self.spec.classes.get(&class).map(|c| c.accountable.clone()).unwrap_or_else(|| self.first_leader());
                self.st.rt.bus.send(&mut self.st.rt.ids, day, "double-loop", vec![to], Intent::Report, "Double-loop finding", x.text.clone(), x.refs.clone(), 1);
            }
            td.rec.findings.extend(f);
        }
        if self.enabled("coherence", day) {
            let rep = ota_tools::coherence::check(&self.spec, &self.st.ledger, &self.st.rt.okrs, day, &mut self.st.rt.ids);
            self.st.rt.coherence_gap = Some(rep.budget_gap);
            td.rec.findings.extend(rep.findings);
        }
        if self.enabled("scenarios", day) {
            let kpis = td.rec.kpis.clone();
            let refuted: Vec<Id> = self.st.assumptions.items.iter().filter(|a| a.status == AssumptionStatus::Refuted).map(|a| a.id.clone()).collect();
            let changes = {
                let ctx = MetricCtx::new(day, &kpis, &self.st.lattice, &self.st.rt.last_seen);
                let mut sc = std::mem::take(&mut self.st.scenarios);
                let ch = sc.review(day, &ctx, &refuted);
                self.st.scenarios = sc;
                ch
            };
            td.rec.scenario_changes.extend(changes);
        }
    }

    // ------------------------------------------------------------------ 11 daily monitors

    pub(crate) fn step_daily_monitors(&mut self, td: &mut Today) {
        let day = td.day;
        // aggregates the monitors read
        let ai: Vec<f64> = td.rec.decisions.iter().filter(|d| d.tier != Tier::Human).map(|d| d.confidence).collect();
        td.agg.ai_mean_confidence = (!ai.is_empty()).then(|| ai.iter().sum::<f64>() / ai.len() as f64);
        td.agg.coherence_gap = self.st.rt.coherence_gap;
        td.agg.double_loop_recurring = self.st.double_loop.recurring_count();
        td.agg.kpis_decaying = self.spec.kpis.iter().filter(|k| k.target.is_some()).filter(|k| {
            match self.st.lattice.trend(&k.metric, day, 28) {
                Some(t) if k.direction == "down" => t > 0.003,
                Some(t) if k.direction == "up" => t < -0.003,
                _ => false,
            }
        }).count();
        let mut pending_agg = std::mem::take(&mut td.agg);
        pending_agg.decisions = td.rec.decisions.len();
        self.st.history.push(pending_agg.clone());

        if self.enabled("sentinel", day) {
            let inp = SentinelInput { day, history: &self.st.history, ledger: &self.st.ledger, calibration: &self.st.calibration,
                                      pilot_limbo_n: self.spec.company.pilot_limbo_n };
            let mut sn = std::mem::take(&mut self.st.sentinel);
            let found = sn.scan(&inp, &mut self.st.rt.ids);
            self.st.sentinel = sn;
            for f in found.iter().filter(|f| f.severity == Severity::High) {
                let to: Vec<NodeId> = self.spec.leaders().map(|p| p.id.clone()).collect();
                self.st.rt.bus.send(&mut self.st.rt.ids, day, "sentinel", to, Intent::Escalate, format!("Sentinel: {}", kind_name(&f.kind)),
                                    f.signature.clone(), vec![f.id.clone()], 1);
            }
            td.rec.sentinel.extend(found);
        }
        td.agg = pending_agg;

        if self.enabled("brief", day) {
            let mut items: Vec<String> = vec![];
            for d in td.rec.deviations.iter().filter(|d| d.level >= 3).take(5) {
                items.push(format!("{} {:+.1}σ ({}){}", d.metric, d.z, d.detector, d.interpretation.as_ref().map(|i| format!(": {i}")).unwrap_or_default()));
            }
            for u in td.rec.assumption_updates.iter().filter(|u| matches!(u.status, AssumptionStatus::Refuted | AssumptionStatus::Stale)) {
                items.push(format!("Assumption {} {}: {}", u.assumption, u.status, u.reason));
            }
            for f in &td.rec.sentinel { items.push(format!("Sentinel {}: {}", kind_name(&f.kind), f.signature)); }
            for o in &td.rec.okr_flags { items.push(format!("OKR {} flagged: {}", o.id, o.flagged.clone().unwrap_or_default())); }
            let blocked = self.st.ledger.decisions.values().filter(|d| d.status == DecisionStatus::Blocked).count();
            if blocked > 0 { items.push(format!("{blocked} decision(s) blocked at the red gate")); }
            for e in td.rec.events.iter().take(3) { items.push(format!("Event: {}", e.title)); }
            let req = LlmRequest {
                tier: "small".into(), purpose: "brief".into(), module: Some("signal-brief".into()),
                system: "Draft the daily Signal Brief for leadership: one headline and at most 6 short items, bad news first, no spin.".into(),
                user: format!("Day {day} ({}). Candidate items: {:?}", td.rec.date, items), schema_name: "brief".into(),
                schema: Some(governance::brief_schema()), context: json!({"day": day, "items": items, "kpis": td.rec.kpis}), day,
            };
            let r = self.llm.call(&req);
            let (headline, list, tier) = match r.value {
                Some(v) => (v["headline"].as_str().unwrap_or("").to_string(),
                            v["items"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_else(|| items.clone()), Tier::Small),
                None => (items.first().cloned().unwrap_or_else(|| "Quiet day".into()), items.clone(), Tier::Rule),
            };
            let b = SignalBrief { day, headline, items: list, tier, approved_by: None, source: None };
            self.st.rt.briefs.push(b.clone());
            if self.st.rt.briefs.len() > 60 { self.st.rt.briefs.remove(0); }
            let leader = self.first_leader();
            self.enqueue(day, &leader, QueueKind::BriefApproval, None, format!("Approve Signal Brief D{day:04}"), json!({"headline": b.headline}), false);
            td.rec.brief = Some(b);
        }
        self.toolkit_runs(td);
    }

    // ------------------------------------------------------------------ 12 monthly

    pub(crate) fn step_monthly(&mut self, td: &mut Today) {
        let day = td.day;
        if self.enabled("meta_loop", day) {
            let ctx = learning::meta_loop_context(&self.st.sentinel, &self.st.rt.research, day);
            let req = LlmRequest {
                tier: "large".into(), purpose: "meta_loop".into(), module: Some("meta-loop".into()),
                system: "You are the Meta-loop. From external research/peer practice and Sentinel's own record, propose at most 3 toolkit changes. You only propose; leadership decides.".into(),
                user: format!("Context: {ctx}"), schema_name: "meta_loop".into(), schema: Some(governance::meta_loop_schema()), context: ctx, day,
            };
            if let Some(v) = self.llm.call(&req).value {
                let leader = self.first_leader();
                for p in v["proposals"].as_array().cloned().unwrap_or_default().into_iter().take(3) {
                    let title = p["title"].as_str().unwrap_or("proposal").to_string();
                    let f = Finding { id: self.st.rt.ids.next("ML"), day, tool: "meta_loop".into(), kind: "proposal".into(),
                                      text: format!("{title}: {}", p["rationale"].as_str().unwrap_or("")), refs: vec![], stats: Default::default() };
                    self.enqueue(day, &leader, QueueKind::MetaLoopProposal, None, format!("Meta-loop proposal: {title}"), json!({"finding": f.id}), false);
                    td.rec.findings.push(f);
                }
            }
        }
        if self.enabled("audit", day) {
            let mut expected: Vec<String> = self.spec.kpis.iter().map(|k| k.metric.clone()).collect();
            expected.extend(self.spec.modules.iter().flat_map(|m| m.inputs.clone()));
            expected.sort();
            expected.dedup();
            let (f, sn) = learning::audit(&self.st.ledger, &self.st.calibration, &self.st.sentinel, &self.st.lattice, &expected,
                                          &self.st.rt.published_module_brier, day, &mut self.st.rt.ids);
            td.rec.findings.extend(f);
            self.st.sentinel.findings.extend(sn.iter().cloned());
            td.rec.sentinel.extend(sn);
        }
        if self.enabled("scenarios", day) {
            let leader = self.first_leader();
            let pending: Vec<(Id, String)> = self.st.scenarios.pending(ScenarioTier::Major).iter().map(|n| (n.id.clone(), n.title.clone())).collect();
            for (id, title) in pending {
                if !self.st.rt.queue.iter().any(|q| q.payload["scenario"].as_str() == Some(&id)) {
                    self.enqueue(day, &leader, QueueKind::ScenarioApproval, None, format!("Scenario meeting: {title}"), json!({"scenario": id}), false);
                }
            }
        }
    }

    // ------------------------------------------------------------------ 13 quarterly

    pub(crate) fn step_quarterly(&mut self, td: &mut Today) {
        let day = td.day;
        if !self.enabled("grid", day) {
            return;
        }
        let date = self.date_of(day);
        let q = format!("{}-Q{}", date.year(), (date.month() - 1) / 3 + 1);
        let enabled: Vec<String> = td.rec.tools_enabled.clone();
        let g = ota_tools::grid::assess(day, q, &enabled, &self.st.history, self.st.ledger.completeness(day.saturating_sub(89), day));
        self.st.rt.grid.push(g.clone());
        td.rec.grid = Some(g);
    }

    // ------------------------------------------------------------------ metrics

    pub(crate) fn compute_metrics(&mut self, td: &mut Today) {
        let day = td.day;
        let committed: Vec<&Decision> = self.st.ledger.decisions.values().filter(|d| d.committed_day == Some(day)).collect();
        let mut m = DayMetrics { decisions_committed: committed.len(), throughput_per_day: committed.len() as f64, ..Default::default() };
        for d in &td.rec.decisions { *m.decisions_by_tier.entry(d.tier.to_string()).or_insert(0) += 1; }
        let lat: Vec<f64> = committed.iter().filter_map(|d| d.first_signal_day.map(|f| (day - f) as f64)).collect();
        m.decision_latency_days = mean(&lat);
        let loops: Vec<f64> = td.rec.actions.iter().filter_map(|a| self.st.ledger.get(&a.decision))
            .map(|d| (day - d.first_signal_day.unwrap_or(d.when)) as f64).collect();
        m.loop_speed_days = mean(&loops);
        let hops: Vec<f64> = td.rec.messages.iter().filter(|x| matches!(x.intent, Intent::Propose | Intent::Escalate | Intent::Request))
            .filter(|x| x.to.iter().any(|t| self.spec.is_human(t))).map(|x| x.hops as f64).collect();
        m.hops_to_decider = mean(&hops);
        let from = day.saturating_sub(29);
        let ha = self.st.history.sum(from, day, |a| a.human_actions) + td.agg.human_actions;
        let rh = self.st.history.sum(from, day, |a| a.real_human_actions) + td.agg.real_human_actions;
        m.real_human_share = (ha > 0).then(|| rh as f64 / ha as f64);
        m.external_signal_share = td.agg.external_share;
        m.human_minutes = td.rec.human_actions.iter().map(|h| h.minutes_spent).sum();
        m.alerts_routed = td.agg.alerts_routed;
        m.alerts_suppressed = td.agg.alerts_suppressed;
        m.premortems_run = td.agg.premortems_run;
        m.premortems_skipped = td.agg.premortems_skipped;
        m.ledger_completeness = self.st.ledger.completeness(from, day);
        m.brier_30d = self.st.calibration.brier_window(from, day, Some(false));
        m.llm_cost_usd = td.rec.llm_calls.iter().map(|c| c.cost_usd).sum();
        m.bad_news_raised = td.rec.assumption_updates.iter().filter(|u| u.status == AssumptionStatus::Refuted).count()
            + td.rec.decision_updates.iter().filter(|d| d.outcome.as_ref().map(|o| o.success == Some(false) && o.sealed_day == day).unwrap_or(false)).count()
            + td.rec.anomalies.len();
        // learning rate (Wright curve) on configured cost KPIs: progress ratio per doubling
        let lrs: Vec<f64> = self.spec.company.learning_curve_kpis.iter().filter_map(|k| wright(self.st.lattice.history.get(k)?)).collect();
        m.learning_rate = mean(&lrs);
        if let Some(gap) = self.st.rt.coherence_gap { m.extra.insert("coherence_gap".into(), gap); }
        m.extra.insert("queue_open".into(), self.st.rt.queue.len() as f64);
        m.extra.insert("stale_assumptions".into(), self.st.assumptions.graveyard().len() as f64);
        td.rec.metrics = m;
    }

    /// A ModuleRun for each toolkit module so the viewer can show tools as nodes.
    fn toolkit_runs(&mut self, td: &mut Today) {
        let day = td.day;
        let mods: Vec<ota_spec::ModuleSpec> = self.spec.modules.iter().filter(|m| m.tool.is_some()).cloned().collect();
        for m in mods {
            let tool = m.tool.clone().unwrap();
            if !self.enabled(&tool, day) { continue; }
            let r = &td.rec;
            let (rationale, outs): (String, Vec<Id>) = match tool.as_str() {
                "lattice" => (format!("{} deviation(s); {} routed, {} over the alert budget", r.deviations.len(), td.agg.alerts_routed, td.agg.alerts_suppressed),
                              r.deviations.iter().map(|d| d.id.clone()).collect()),
                "assumptions" => (format!("{} assumption change(s); {} in the graveyard", r.assumption_updates.len(), td.agg.stale_assumptions),
                                  r.assumption_updates.iter().map(|u| u.assumption.clone()).collect()),
                "premortem" => (format!("{} run, {} skipped", td.agg.premortems_run, td.agg.premortems_skipped), r.premortems.iter().map(|p| p.id.clone()).collect()),
                "adversarial" => (format!("{} challenge(s)", r.challenges.len()), r.challenges.iter().map(|c| c.id.clone()).collect()),
                "ledger" => (format!("{} new decision(s), {} updated", r.decisions.len(), r.decision_updates.len()), r.decisions.iter().map(|d| d.id.clone()).collect()),
                "sentinel" => (format!("{} finding(s)", r.sentinel.len()), r.sentinel.iter().map(|s| s.id.clone()).collect()),
                "brief" => (r.brief.as_ref().map(|b| b.headline.clone()).unwrap_or_default(), vec![]),
                "gate" => (format!("{} gated decision(s)", r.decisions.iter().filter(|d| d.gate.as_ref().map(|g| g.color != GateColor::Green).unwrap_or(false)).count()), vec![]),
                "anomalies" => (format!("{} anomaly entries", r.anomalies.len()), r.anomalies.iter().map(|a| a.id.clone()).collect()),
                "scenarios" => (format!("{} scenario change(s)", r.scenario_changes.len()), r.scenario_changes.iter().map(|s| s.id.clone()).collect()),
                "okrs" => (format!("{} OKR flag(s)", r.okr_flags.len()), r.okr_flags.iter().map(|o| o.id.clone()).collect()),
                "calibration" => (format!("{} forecast(s) made, {} resolved", r.forecasts_made.len(), r.forecasts_resolved.len()), r.forecasts_resolved.iter().map(|f| f.id.clone()).collect()),
                "autonomy" => (format!("{} autonomy change(s)", r.autonomy_changes.len()), r.autonomy_changes.iter().map(|a| a.id.clone()).collect()),
                t => {
                    let f: Vec<&Finding> = r.findings.iter().filter(|f| f.tool == t).collect();
                    (format!("{} finding(s)", f.len()), f.iter().map(|x| x.id.clone()).collect())
                }
            };
            let msgs: Vec<Id> = self.st.rt.bus.today.iter().filter(|x| x.from == m.id).map(|x| x.id.clone()).collect();
            td.rec.module_runs.push(ModuleRun {
                day, module: m.id.clone(),
                think: ThinkTrace { tier: Some(Tier::Rule), rationale: format!("[{tool}] {rationale}"), ..Default::default() },
                outputs: RunOutputs { messages: msgs, anomalies: vec![], decisions: if tool == "ledger" { outs.clone() } else { vec![] },
                                      forecasts: if tool == "calibration" { outs.clone() } else { vec![] }, ..Default::default() },
                inputs: RunInputs { deviations: if tool == "lattice" { outs } else { vec![] }, ..Default::default() },
                ..Default::default()
            });
        }
    }
}

fn mean(v: &[f64]) -> Option<f64> {
    (!v.is_empty()).then(|| v.iter().sum::<f64>() / v.len() as f64)
}

/// Wright learning-curve progress ratio (cost multiplier per doubling of cumulative
/// experience, using days as the experience proxy). < 1 means learning.
fn wright(h: &[(Day, f64)]) -> Option<f64> {
    let pts: Vec<(f64, f64)> = h.iter().filter(|(_, v)| *v > 0.0).map(|(d, v)| ((*d as f64).ln(), v.ln())).collect();
    if pts.len() < 30 { return None; }
    let n = pts.len() as f64;
    let mx = pts.iter().map(|p| p.0).sum::<f64>() / n;
    let my = pts.iter().map(|p| p.1).sum::<f64>() / n;
    let sxy: f64 = pts.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
    let sxx: f64 = pts.iter().map(|p| (p.0 - mx).powi(2)).sum();
    (sxx > 0.0).then(|| 2f64.powf(sxy / sxx))
}

pub(crate) fn kind_name(k: &FindingKind) -> String {
    match k {
        FindingKind::Pathogen(p) => p.to_string(),
        FindingKind::FailureMode(f) => f.to_string(),
        FindingKind::SignalClass(s) => s.to_string(),
        FindingKind::Rule(r) => r.clone(),
    }
}

/// `data/state/view.json` — the `/api/state` payload (see docs/DATA.md).
pub(crate) fn state_view(e: &Engine) -> Value {
    let st = &e.st;
    let cal = st.calibration.public(&|f| (Some(f.by.clone()), Some(f.topic.clone())));
    json!({
        "day": st.day,
        "date": e.date_of(st.day.max(1)).format("%Y-%m-%d").to_string(),
        "toolkit": e.spec.toolkit.iter().map(|t| json!({"id": t.id, "enabled_from": t.enabled_from, "enabled": t.enabled_from <= st.day,
                                                         "depends_on": t.depends_on, "evolves_from": t.evolves_from})).collect::<Vec<_>>(),
        "assumptions": st.assumptions.items,
        "graveyard": st.assumptions.graveyard().iter().map(|a| a.id.clone()).collect::<Vec<_>>(),
        "scenarios": st.scenarios.nodes,
        "autonomy": {"rungs": st.autonomy.rungs, "history": st.autonomy.history},
        "calibration": cal,
        "okrs": st.rt.okrs,
        "sentinel": st.sentinel.findings.iter().rev().take(300).collect::<Vec<_>>(),
        "findings": st.rt.findings,
        "coherence_gap": st.rt.coherence_gap,
        "grid": st.rt.grid,
        "briefs": st.rt.briefs.iter().rev().take(14).collect::<Vec<_>>(),
        "queue": st.rt.queue,
        "alert_queue": st.rt.alert_queue.iter().map(|(k, v)| (k.clone(), v.len())).collect::<BTreeMap<_, _>>(),
        "anomaly_clusters": st.anomalies.promoted,
        "double_loop_recurring": st.double_loop.recurring,
        "ledger_size": st.ledger.decisions.len(),
        "llm_spent_total_usd": e.llm.spent_total,
    })
}
