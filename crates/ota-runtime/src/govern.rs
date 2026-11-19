//! Step 7 (governance) and step 8 (human stand-ins), plus live real-human input.

use crate::state::HumanInput;
use crate::tick::Today;
use crate::Engine;
use ota_core::*;
use ota_llm::LlmRequest;
use ota_tools::governance::{self, challenges_from, gate_classify, material_assumptions, premortem_from, premortem_needed};
use ota_tools::history::mean_pairwise_similarity;
use ota_tools::lattice::MetricCtx;
use ota_tools::sentinel::{HIGH_STAKES_EUR, MIN_HUMAN_MINUTES_HIGH_STAKES};
use serde_json::json;

fn decision_brief(d: &Decision) -> serde_json::Value {
    json!({"id": d.id, "class": d.class, "module": d.module, "what": d.what, "action": d.action, "stakes_eur": d.stakes_eur,
           "novelty": d.novelty, "confidence": d.confidence, "reversibility": d.reversibility, "impact": d.impact,
           "kind": d.kind, "tier": d.tier, "reasoning": d.reasoning, "assumptions": d.assumptions, "expected": d.expected,
           "switches": d.switches, "action_elements": d.action_elements})
}

impl Engine {
    // ------------------------------------------------------------------ 7 governance

    pub(crate) fn step_governance(&mut self, td: &mut Today) {
        let day = td.day;
        let proposals = std::mem::take(&mut td.proposals);
        let threshold = self.spec.company.premortem_threshold;
        let mut fm_texts: Vec<String> = vec![];
        for mut d in proposals {
            let Some(c) = self.spec.classes.get(&d.class).cloned() else { continue };
            let ts = self.st.rt.bus.tick(5);

            // (b) automatic pre-mortem
            if premortem_needed(d.stakes_eur, d.novelty, threshold) {
                let started = self.st.rt.bus.tick(1);
                d.premortem_started_ts = Some(started);
                let pm_id = self.st.rt.ids.next("PM");
                let mut pm = PreMortem { id: pm_id.clone(), decision: d.id.clone(), day, started_ts: started, failure_modes: vec![],
                                         load_bearing: d.assumptions.clone(), skipped: true, skip_reason: None, tier: Tier::Rule };
                if self.enabled("premortem", day) {
                    let req = LlmRequest {
                        tier: "large".into(), purpose: "premortem".into(), module: Some(d.module.clone()),
                        system: "You run an automatic pre-mortem. Assume it is the check date and this decision failed. List the most plausible distinct failure modes with probability and severity (0..1), and the load-bearing assumptions. Estimate the minutes a careful team would spend.".into(),
                        user: format!("Decision: {}", decision_brief(&d)), schema_name: "premortem".into(), schema: Some(governance::premortem_schema()),
                        context: json!({"decision": decision_brief(&d), "class": c.id}), day,
                    };
                    let r = self.llm.call(&req);
                    match r.value {
                        Some(v) => {
                            let tier = if r.record.tier == "small" { Tier::Small } else { Tier::Large };
                            pm = premortem_from(&v, pm_id.clone(), &d, started, tier);
                            let minutes = v["minutes"].as_f64().unwrap_or(45.0).clamp(0.0, 600.0) as u32;
                            self.st.rt.bus.tick(minutes);
                            fm_texts.extend(pm.failure_modes.iter().map(|f| f.text.clone()));
                            td.agg.premortems_run += 1;
                        }
                        None => {
                            pm.skip_reason = Some("LLM budget exhausted".into());
                            td.agg.premortems_skipped += 1;
                        }
                    }
                } else {
                    pm.skip_reason = Some("auto pre-mortem not enabled yet (layered rollout)".into());
                    td.agg.premortems_skipped += 1;
                }
                d.premortem = Some(pm_id);
                td.rec.premortems.push(pm);
            }

            // (c) adversarial engine
            if self.enabled("adversarial", day) {
                let mats: Vec<Assumption> = material_assumptions(&d, &self.st.assumptions).into_iter().cloned().collect();
                if !mats.is_empty() && (d.path == DecisionPath::Path2 || d.novelty >= 0.3 || d.stakes_eur >= threshold / 2.0) {
                    let av: Vec<serde_json::Value> = mats.iter().map(|a| json!({"id": a.id, "text": a.text, "confidence": a.confidence, "load_bearing": a.load_bearing})).collect();
                    let req = LlmRequest {
                        tier: "large".into(), purpose: "adversarial".into(), module: Some("adversarial-engine".into()),
                        system: "You are the Adversarial Engine. For each material assumption build the strongest honest counter-argument, cite the kind of evidence, and rate its strength 0..1.".into(),
                        user: format!("Decision: {}\nMaterial assumptions: {}", decision_brief(&d), serde_json::Value::Array(av.clone())),
                        schema_name: "challenges".into(), schema: Some(governance::challenge_schema()),
                        context: json!({"decision": decision_brief(&d), "assumptions": av}), day,
                    };
                    if let Some(v) = self.llm.call(&req).value {
                        let mut chs = challenges_from(&v, &d, day, &mut self.st.rt.ids);
                        for ch in chs.iter_mut() {
                            if ch.strength > d.confidence {
                                d.confidence = (d.confidence * 0.9).max(0.05);
                                ch.response = Some(format!("{}: confidence reduced to {:.2}; switches cover it", d.module, d.confidence));
                            } else {
                                ch.response = Some(format!("{}: noted; stop signal `{}` covers this risk", d.module, d.switches.stop_if));
                            }
                            d.challenges.push(ch.id.clone());
                        }
                        td.agg.challenges += chs.len();
                        for ch in &chs { self.st.rt.challenges.insert(ch.id.clone(), ch.clone()); }
                        td.rec.challenges.extend(chs);
                    }
                }
            }

            // (d) domain expert gate
            let mut blocked = false;
            if self.enabled("gate", day) {
                let kpis = td.rec.kpis.clone();
                let g = {
                    let b = |x: bool| if x { 1.0 } else { 0.0 };
                    let ctx = MetricCtx::new(day, &kpis, &self.st.lattice, &self.st.rt.last_seen)
                        .with("stakes_eur", d.stakes_eur).with("novelty", d.novelty).with("confidence", d.confidence)
                        .with("reversible", b(d.reversibility == Reversibility::Reversible))
                        .with("irreversible", b(d.reversibility == Reversibility::Irreversible))
                        .with("company_wide", b(d.impact == Impact::CompanyWide)).with("path2", b(d.path == DecisionPath::Path2));
                    gate_classify(&self.spec, &c, &ctx)
                };
                let mut g = g;
                match g.color {
                    GateColor::Yellow => {
                        g.recommendation = Some("validator notified; decision proceeds".into());
                        if let Some(v) = &g.validator {
                            self.st.rt.bus.send(&mut self.st.rt.ids, day, "domain-expert-gate", vec![v.clone()], Intent::Inform,
                                                format!("Yellow gate: {}", d.what), g.rule.clone(), vec![d.id.clone()], 1);
                        }
                    }
                    GateColor::Red => {
                        let ai_may_review = d.path == DecisionPath::Path2 && d.rung_at_time >= Rung::ActThenReview && !c.human_only;
                        if ai_may_review {
                            g.reviewed_by = Some("ai".into());
                            g.reviewed_day = Some(day);
                            g.verdict = Some(HumanVerdict::Approve);
                            g.recommendation = Some("reviewed by AI under earned Path 2 authority".into());
                        } else {
                            blocked = true;
                        }
                    }
                    GateColor::Green => {}
                }
                d.gate = Some(g);
            }

            // (e) autonomy rung / (f) ledger / (g) impact
            td.rec.decisions.push(d.clone());
            if blocked {
                d.status = DecisionStatus::Blocked;
                let v = d.gate.as_ref().and_then(|g| g.validator.clone()).unwrap_or_else(|| self.first_leader());
                self.enqueue(day, &v, QueueKind::GateReview, Some(d.id.clone()), format!("Red gate: {}", d.what), json!({"rule": d.gate.as_ref().map(|g| g.rule.clone())}), true);
                self.st.ledger.insert(d);
            } else {
                self.st.ledger.insert(d.clone());
                self.route_by_rung(td, &d.id, ts);
            }
        }
        td.agg.premortem_similarity = mean_pairwise_similarity(&fm_texts);
    }

    /// Execute, or ask for approval, according to the autonomy rung.
    pub(crate) fn route_by_rung(&mut self, td: &mut Today, id: &str, ts: Ts) {
        let day = td.day;
        let Some(d) = self.st.ledger.get(id).cloned() else { return };
        if d.rung_at_time.executes_without_approval() && d.tier != Tier::Human {
            self.commit(td, id, ts, None);
            if d.rung_at_time == Rung::ActThenReview {
                // humans review patterns afterwards: sample one in four (plus every high-stakes item)
                let h: u32 = id.bytes().map(|b| b as u32).sum();
                if h % 4 == 0 || d.stakes_eur >= HIGH_STAKES_EUR {
                    self.enqueue(day, &d.accountable, QueueKind::Review, Some(d.id.clone()), format!("Post-hoc review: {}", d.what), json!({}), false);
                }
            }
        } else {
            if let Some(x) = self.st.ledger.get_mut(id) { x.status = DecisionStatus::Proposed; }
            let verb = if d.rung_at_time <= Rung::Recommend { "Recommendation" } else { "Approval needed" };
            self.enqueue(day, &d.accountable, QueueKind::Approval, Some(d.id.clone()), format!("{verb}: {}", d.what), json!({}), true);
            self.st.rt.bus.send(&mut self.st.rt.ids, day, &d.module, vec![d.accountable.clone()], Intent::Propose, format!("{verb}: {}", d.what),
                                d.reasoning.clone(), vec![d.id.clone()], 1);
        }
    }

    /// Commit a decision: approved, forecasts registered, stakeholders informed.
    pub(crate) fn commit(&mut self, td: &mut Today, id: &str, ts: Ts, approver: Option<&str>) {
        let day = td.day;
        let Some(mut d) = self.st.ledger.get(id).cloned() else { return };
        d.status = DecisionStatus::Approved;
        d.committed_ts = Some(ts.max(d.premortem_started_ts.unwrap_or(0)));
        d.committed_day = Some(day);
        if let Some(a) = approver { d.who = a.to_string(); }
        if let (Some(s), Some(c)) = (d.premortem_started_ts, d.committed_ts) {
            if d.committed_day == Some(d.when) {
                let gap = c.saturating_sub(s) as f64;
                td.agg.laundering_gap_min = Some(td.agg.laundering_gap_min.map(|g| (g + gap) / 2.0).unwrap_or(gap));
            }
        }
        // forecast registered with the calibration tracker
        if let Some(c) = self.spec.classes.get(&d.class) {
            let f = Forecast {
                id: self.st.rt.ids.next("F"), day, by: d.module.clone(),
                question: c.forecast.clone().unwrap_or_else(|| format!("{} confirms by day {}", d.id, d.switches.check_by)),
                p: d.confidence, resolves_on: day + if c.forecast_if.is_some() { c.forecast_horizon_days.max(1) } else { c.check_after_days.max(1) },
                outcome: None, brier: None, topic: d.class.clone(), horizon_days: c.forecast_horizon_days, decision: Some(d.id.clone()),
                resolve_if: c.forecast_if.clone(), human: false,
            };
            d.forecasts.push(f.id.clone());
            td.rec.forecasts_made.push(f.clone());
            self.st.calibration.add(f);
            if c.requires_action_elements() && !d.action_elements.missing().is_empty() {
                td.agg.missing_action_elements += 1;
                d.notes.push(format!("missing action elements: {}", d.action_elements.missing().join(", ")));
            }
        }
        // the decider step: cited deviations reach L6
        for i in &d.information {
            if let Some(t) = self.st.history.devs.iter_mut().find(|t| &t.id == i) {
                if t.acted_day.is_none() { t.acted_day = Some(day); }
            }
            if let Some(dv) = td.rec.deviations.iter_mut().find(|x| &x.id == i) { dv.level = 6; }
        }
        if d.confidence >= 0.8 && d.information.len() <= 1 { td.agg.single_source_high_conf += 1; }
        if d.accountable.is_empty() || !self.spec.is_human(&d.accountable) { td.agg.no_accountable += 1; }
        if d.approvals.len() >= 3 && !d.approvals.iter().any(|a| a.by == d.accountable) { td.agg.diffuse_approvals += 1; }
        // (g) impact decides who is informed
        let mut informed = vec![d.accountable.clone()];
        match d.impact {
            Impact::Local => {}
            Impact::CrossFunction => {
                informed.extend(self.spec.modules.iter().filter(|m| m.depends_on.contains(&d.module) || m.org == self.spec.module(&d.module).map(|x| x.org.clone()).unwrap_or_default())
                    .filter(|m| m.id != d.module).map(|m| m.id.clone()));
            }
            Impact::CompanyWide => {
                informed.extend(self.spec.leaders().map(|p| p.id.clone()));
                informed.extend(self.spec.modules.iter().filter(|m| m.id != d.module).map(|m| m.id.clone()));
            }
        }
        informed.sort();
        informed.dedup();
        d.informed = informed.clone();
        if d.impact != Impact::Local || d.tier != Tier::Rule {
            self.st.rt.bus.send(&mut self.st.rt.ids, day, &d.module, informed, Intent::Inform, format!("Committed: {}", d.what),
                                d.expected.clone(), vec![d.id.clone()], 1);
        }
        if d.when != day { td.touched.insert(d.id.clone()); }
        self.st.ledger.insert(d);
        td.to_execute.push(id.to_string());
    }

    // ------------------------------------------------------------------ 8 human stand-ins

    pub(crate) fn step_humans(&mut self, td: &mut Today) {
        let day = td.day;
        let people = self.spec.people.clone();
        for p in &people {
            // leader think week: first week of each quarter is protected (capacity blocked)
            let think_week = p.role == NodeKind::Leader && td.quarter_start;
            let capacity = if think_week { 0.0 } else { p.weekly_capacity_min / 5.0 };
            let mut used = 0.0;
            let mut items: Vec<QueueItem> = self.st.rt.queue.iter().filter(|q| q.person == p.id).cloned().collect();
            items.sort_by_key(|q| (!q.blocking, q.created_day));
            for item in items {
                if used >= capacity { break; }
                let d = item.decision.as_ref().and_then(|i| self.st.ledger.get(i)).cloned();
                let chs: Vec<&Challenge> = d.as_ref().map(|d| d.challenges.iter().filter_map(|c| self.st.rt.challenges.get(c)).collect()).unwrap_or_default();
                let pm = d.as_ref().and_then(|d| d.premortem.as_ref()).and_then(|id| td.rec.premortems.iter().find(|x| &x.id == id));
                let ctx = json!({
                    "person": {"id": p.id, "role": p.role, "title": p.title, "bias": p.bias, "expertise": p.expertise},
                    "item": {"id": item.id, "kind": item.kind, "subject": item.subject, "payload": item.payload, "age_days": day - item.created_day},
                    "decision": d.as_ref().map(decision_brief), "gate": d.as_ref().and_then(|d| d.gate.clone()),
                    "challenges": chs, "premortem": pm,
                });
                let req = LlmRequest {
                    tier: p.llm_tier.clone(), purpose: "standin".into(), module: Some(p.id.clone()),
                    system: format!("You are a stand-in for {} ({}, {}). Persona:\n{}\n\nHow you judge:\n{}\n\nBias profile: overconfidence {:.2}, challenge acceptance {:.2}, review diligence {:.2}. Decide as this person would. Never copy the AI's reasoning as your reason; give your own.",
                                    p.name, p.role, p.title, p.persona, p.judgment, p.bias.overconfidence, p.bias.challenge_acceptance, p.bias.review_diligence),
                    user: format!("Queue item: {}", ctx), schema_name: "standin_response".into(), schema: Some(governance::standin_schema()),
                    context: ctx, day,
                };
                let r = self.llm.call(&req);
                let v = r.value.unwrap_or_else(|| json!({"decision": "defer", "reason": "no capacity (LLM budget)", "minutes_spent": 0}));
                let verdict: HumanVerdict = v["decision"].as_str().and_then(|s| s.parse().ok()).unwrap_or(HumanVerdict::Defer);
                let minutes = v["minutes_spent"].as_f64().unwrap_or(5.0).max(0.0);
                used += minutes;
                self.resolve_item(td, &item, p.id.clone(), verdict, v["reason"].as_str().unwrap_or("").into(),
                                  v["amendments"].as_str().filter(|s| !s.is_empty()).map(String::from), minutes,
                                  v["confidence"].as_f64().unwrap_or(0.6), v["manual_check"].as_bool().unwrap_or(false), HumanSource::Standin);
            }
            if used > 0.0 { td.rec.human_load.insert(p.id.clone(), used); }
        }
    }

    /// Apply a human verdict to a queue item (stand-in or real human).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn resolve_item(&mut self, td: &mut Today, item: &QueueItem, person: NodeId, verdict: HumanVerdict, reason: String,
                               amendments: Option<String>, minutes: f64, confidence: f64, manual_check: bool, source: HumanSource) {
        let day = td.day;
        let ts = self.st.rt.bus.tick(minutes.round() as u32);
        td.rec.human_actions.push(HumanAction {
            id: self.st.rt.ids.next("H"), day, ts, person: person.clone(), item: item.id.clone(), kind: item.kind, decision: item.decision.clone(),
            verdict, reason: reason.clone(), amendments: amendments.clone(), minutes_spent: minutes, confidence, source, manual_check,
        });
        td.agg.human_actions += 1;
        td.agg.human_minutes += minutes;
        if manual_check { td.agg.manual_checks += 1; }
        if source == HumanSource::RealHuman { td.agg.real_human_actions += 1; }
        if verdict == HumanVerdict::Defer {
            return; // stays queued
        }
        self.st.rt.queue.retain(|q| q.id != item.id);
        let approval = Approval { by: person.clone(), day, ts, verdict, reason: reason.clone(), minutes_spent: minutes, source,
                                  amendments: amendments.clone(), post_hoc: item.kind == QueueKind::Review };
        match item.kind {
            QueueKind::Approval | QueueKind::GateReview | QueueKind::Review if item.decision.is_some() => {
                let id = item.decision.clone().unwrap();
                let Some(mut d) = self.st.ledger.get(&id).cloned() else { return };
                if mean_pairwise_similarity(&[reason.clone(), d.reasoning.clone()]).map(|s| s > 0.8).unwrap_or(false) { td.agg.copied_reasons += 1; }
                if d.stakes_eur >= HIGH_STAKES_EUR && verdict != HumanVerdict::Reject {
                    td.agg.high_stakes_approvals += 1;
                    if minutes < MIN_HUMAN_MINUTES_HIGH_STAKES { td.agg.fast_high_stakes_approvals += 1; }
                }
                if d.when != day { td.touched.insert(id.clone()); }
                let approve = matches!(verdict, HumanVerdict::Approve | HumanVerdict::Amend);
                if verdict == HumanVerdict::Amend { *td.agg.overrides.entry(format!("amend:{}", d.class)).or_insert(0) += 1; }
                match item.kind {
                    QueueKind::GateReview => {
                        if let Some(g) = d.gate.as_mut() {
                            g.reviewed_by = Some(person.clone());
                            g.reviewed_day = Some(day);
                            g.verdict = Some(verdict);
                        }
                        d.approvals.push(approval);
                        if approve {
                            d.status = DecisionStatus::Proposed;
                            self.st.ledger.insert(d);
                            self.route_by_rung(td, &id, ts);
                        } else {
                            d.status = DecisionStatus::Rejected;
                            td.agg.human_rejections += 1;
                            self.st.ledger.insert(d);
                        }
                    }
                    QueueKind::Review => {
                        if !approve {
                            d.overrides.push(Override { by: person.clone(), day, what: "post_hoc_review".into(), reason: reason.clone(), source });
                            td.agg.human_rejections += 1;
                        }
                        d.approvals.push(approval);
                        self.st.ledger.insert(d);
                    }
                    _ => {
                        // challenges: approving over a strong challenge is an override (groupthink signal)
                        let strong: Vec<Id> = d.challenges.iter().filter(|c| self.st.rt.challenges.get(*c).map(|x| x.strength >= 0.6).unwrap_or(false)).cloned().collect();
                        for cid in &strong {
                            let Some(c) = self.st.rt.challenges.get_mut(cid) else { continue };
                            if approve {
                                c.overridden_by = Some(person.clone());
                                c.accepted = Some(false);
                            } else {
                                c.accepted = Some(true);
                            }
                            let c = c.clone();
                            match td.rec.challenges.iter_mut().find(|x| x.id == c.id) {
                                Some(x) => *x = c,
                                None => td.rec.challenges.push(c),
                            }
                        }
                        if approve {
                            for c in &strong {
                                d.overrides.push(Override { by: person.clone(), day, what: format!("challenge:{c}"), reason: reason.clone(), source });
                                td.agg.challenges_overridden += 1;
                                *td.agg.overrides.entry("challenge".into()).or_insert(0) += 1;
                            }
                        }
                        d.approvals.push(approval);
                        if approve {
                            if verdict == HumanVerdict::Amend { d.notes.push(format!("amended by {person}: {}", amendments.clone().unwrap_or_default())); }
                            d.tier = if d.tier == Tier::Rule && d.rung_at_time <= Rung::Recommend { Tier::Human } else { d.tier };
                            self.st.ledger.insert(d);
                            // the human's own forecast feeds their private calibration profile
                            let f = Forecast { id: self.st.rt.ids.next("F"), day, by: person.clone(), question: format!("{id} succeeds"),
                                               p: confidence.clamp(0.0, 1.0), resolves_on: self.st.ledger.get(&id).map(|x| x.switches.check_by).unwrap_or(day + 7),
                                               outcome: None, brier: None, topic: "approval".into(), horizon_days: 7, decision: Some(id.clone()),
                                               resolve_if: None, human: true };
                            self.st.calibration.add(f);
                            self.commit(td, &id, ts, Some(&person));
                        } else {
                            d.status = DecisionStatus::Rejected;
                            d.who = person.clone();
                            td.agg.human_rejections += 1;
                            self.st.ledger.insert(d);
                        }
                    }
                }
            }
            QueueKind::Review => {
                if let Some(a) = item.payload["assumption"].as_str() {
                    if matches!(verdict, HumanVerdict::Approve | HumanVerdict::Amend) {
                        self.st.assumptions.renew(a, day, &person);
                    } else {
                        self.st.assumptions.archive(a, day, &person);
                    }
                }
            }
            QueueKind::AutonomyChange => {
                if let Some(id) = item.payload["change"].as_str() {
                    if let Some(ch) = self.st.autonomy.resolve(id, verdict == HumanVerdict::Approve, &person, source) {
                        td.rec.autonomy_changes.push(ch);
                    }
                }
            }
            QueueKind::ScenarioApproval => {
                if let Some(id) = item.payload["scenario"].as_str() {
                    if let Some(n) = self.st.scenarios.decide(id, verdict == HumanVerdict::Approve, &person, day) {
                        td.rec.scenario_changes.push(n);
                    }
                }
            }
            QueueKind::BriefApproval => {
                if let Some(b) = self.st.rt.briefs.iter_mut().rev().find(|b| b.day == item.created_day) {
                    if verdict == HumanVerdict::Approve || verdict == HumanVerdict::Amend {
                        b.approved_by = Some(person.clone());
                        b.source = Some(source);
                    }
                }
            }
            QueueKind::MetaLoopProposal => {
                td.rec.findings.push(Finding { id: self.st.rt.ids.next("ML"), day, tool: "meta_loop".into(), kind: format!("proposal_{verdict}"),
                                               text: format!("{}: {} — {reason}", person, item.subject), refs: vec![item.id.clone()], stats: Default::default() });
            }
            _ => {}
        }
    }

    /// Live input from a real human (viewer, admin token). Pre-empts the stand-in.
    pub(crate) fn apply_human_input(&mut self, td: &mut Today, h: HumanInput) {
        let day = td.day;
        if let Some(text) = &h.framing {
            let to = if h.to.is_empty() { self.spec.modules.iter().map(|m| m.id.clone()).collect() } else { h.to.clone() };
            let mid = self.st.rt.bus.send(&mut self.st.rt.ids, day, &h.person, to, Intent::Request, "Framing (real human)", text.clone(), vec![], 0);
            td.rec.human_actions.push(HumanAction { id: self.st.rt.ids.next("H"), day, ts: self.st.rt.bus.clock, person: h.person.clone(),
                item: mid, kind: QueueKind::Framing, decision: None, verdict: HumanVerdict::Approve, reason: text.clone(), amendments: None,
                minutes_spent: 0.0, confidence: 1.0, source: HumanSource::RealHuman, manual_check: true });
            td.agg.real_human_actions += 1;
            td.agg.human_actions += 1;
            td.agg.bad_news_by_humans += 1;
        }
        let Some(verdict) = h.verdict else { return };
        let item = self.st.rt.queue.iter().find(|q| Some(&q.id) == h.item.as_ref() || (h.item.is_none() && q.decision.is_some() && q.decision == h.decision)).cloned();
        match item {
            Some(it) => self.resolve_item(td, &it, h.person.clone(), verdict, h.reason.clone(), None, 0.0, 0.7, true, HumanSource::RealHuman),
            None => {
                if let Some(d) = h.decision.as_ref().and_then(|id| self.st.ledger.get_mut(id)) {
                    d.overrides.push(Override { by: h.person.clone(), day, what: format!("real_human_{verdict}"), reason: h.reason.clone(), source: HumanSource::RealHuman });
                    td.touched.insert(d.id.clone());
                    td.agg.real_human_actions += 1;
                }
            }
        }
    }
}
