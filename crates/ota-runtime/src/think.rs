//! Step 6: module think. Rules first (tier `Rule`); escalations are batched into ONE
//! LLM call per module per day returning a JSON array of decisions (§5).

use crate::tick::Today;
use crate::{module_order, Engine};
use ota_core::*;
use ota_decide::{parse_then, Table};
use ota_llm::LlmRequest;
use ota_spec::{DecisionClass, ModuleSpec};
use ota_tools::governance;
use ota_tools::history::mean_pairwise_similarity;
use ota_tools::lattice::MetricCtx;
use serde_json::{json, Value};
use std::collections::BTreeMap;

struct Escalation {
    class: String,
    hit: RuleHit,
    action: ActionSpec,
    tier: Tier,
    novelty: f64,
    info: Vec<Id>,
}

pub(crate) fn cadence_fires(c: Cadence, td: &Today) -> bool {
    match c {
        Cadence::Daily => true,
        Cadence::Weekly => td.monday,
        Cadence::Monthly => td.month_start,
        Cadence::Quarterly => td.quarter_start,
    }
}

fn render_prompt(tpl: &str, vars: &[(&str, String)]) -> String {
    let mut s = tpl.to_string();
    for (k, v) in vars {
        s = s.replace(&format!("{{{{{k}}}}}"), v);
    }
    s
}

pub(crate) fn describe_action(a: &ActionSpec) -> String {
    if a.is_hold() {
        return "hold".into();
    }
    if a.kind == "adjust" {
        let p = &a.params;
        return match p["op"].as_str() {
            Some("set") => format!("{} = {}", p["target"].as_str().unwrap_or("?"), p["value"]),
            Some(op) => format!("{} {op}= {}", p["target"].as_str().unwrap_or("?"), p["value"]),
            None => format!("adjust {p}"),
        };
    }
    match &a.params {
        Value::Object(m) if !m.is_empty() => format!("{}({})", a.kind, m.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(", ")),
        _ => a.kind.clone(),
    }
}

fn legible(hit: &RuleHit) -> String {
    let b: Vec<String> = hit.bindings.iter().map(|(k, v)| format!("{k}={}", fmt_num(*v))).collect();
    format!("Rule {} row {} fired: when `{}`{} → `{}` (confidence {:.2}).", hit.class, hit.row + 1, hit.when,
            if b.is_empty() { String::new() } else { format!(" with {}", b.join(", ")) }, hit.then, hit.confidence)
}

fn fmt_num(v: f64) -> String {
    if v.abs() >= 100.0 { format!("{v:.0}") } else { format!("{v:.3}") }
}

impl Engine {
    pub(crate) fn step_think(&mut self, td: &mut Today) {
        let mut rationales = vec![];
        for mid in module_order(&self.spec) {
            let m = self.spec.module(&mid).cloned().unwrap();
            if !cadence_fires(m.cadence, td) || m.classes.is_empty() {
                continue;
            }
            let run = self.think_module(&m, td);
            if !run.think.rationale.is_empty() {
                rationales.push(run.think.rationale.clone());
            }
            td.rec.module_runs.push(run);
        }
        td.agg.module_similarity = mean_pairwise_similarity(&rationales);
    }

    fn think_module(&mut self, m: &ModuleSpec, td: &mut Today) -> ModuleRun {
        let t0 = std::time::Instant::now();
        let day = td.day;
        let mut run = ModuleRun { day, module: m.id.clone(), ..Default::default() };
        // ---- inputs (observe)
        run.inputs.deviations = td.routed.get(&m.id).cloned().unwrap_or_default();
        run.inputs.queued_alerts = self.st.rt.alert_queue.remove(&m.id).unwrap_or_default();
        let inbox = self.st.rt.bus.take(&m.id);
        run.inputs.messages = inbox.iter().map(|x| x.id.clone()).collect();
        run.inputs.due_switches = td.sealed.get(&m.id).cloned().unwrap_or_default();
        run.inputs.assumption_flags = td.assumption_flags.get(&m.id).cloned().unwrap_or_default();
        run.inputs.okr_flags = self.st.rt.okrs.iter()
            .filter(|o| td.okr_flags.contains(&o.id) && o.key_results.iter().any(|k| m.inputs.contains(&k.metric)))
            .map(|o| o.id.clone()).collect();
        for i in &m.inputs {
            if let Some(v) = td.rec.kpis.get(i) {
                run.inputs.metrics.insert(i.clone(), *v);
            }
        }
        let inbox_devs: Vec<Deviation> = run.inputs.deviations.iter().chain(run.inputs.queued_alerts.iter())
            .filter_map(|id| td.rec.deviations.iter().find(|d| &d.id == id).cloned().or_else(|| self.st.rt.open_devs.get(id).cloned()))
            .collect();

        // ---- rules (tier Rule)
        let kpis = td.rec.kpis.clone();
        let mut classes: Vec<DecisionClass> = m.classes.iter().filter_map(|c| self.spec.classes.get(c).cloned()).collect();
        classes.sort_by(|a, b| a.id.cmp(&b.id));
        let mut drafts: Vec<Decision> = vec![];
        let mut escalations: Vec<Escalation> = vec![];
        let mut acted_metrics: Vec<String> = vec![];
        for c in &classes {
            let table = match Table::compile(&c.id, &c.rows) {
                Ok(t) => t,
                Err((i, e)) => {
                    run.think.rules_failed.push(format!("{} row {}: {e}", c.id, i + 1));
                    continue;
                }
            };
            let (ev, action, metrics) = {
                let ctx = MetricCtx::new(day, &kpis, &self.st.lattice, &self.st.rt.last_seen);
                let ev = table.evaluate(&ctx);
                let action = ev.hit.as_ref().map(|h| parse_then(&h.then, &ctx));
                (ev, action, table.metrics())
            };
            run.think.rules_failed.extend(ev.errors);
            let Some(hit) = ev.hit else { continue };
            run.think.rules_fired.push(hit.clone());
            let action = match action.unwrap() {
                Ok(a) => a,
                Err(e) => {
                    run.think.rules_failed.push(format!("{} row {} action: {e}", c.id, hit.row + 1));
                    ActionSpec::hold()
                }
            };
            let info: Vec<Id> = inbox_devs.iter().filter(|d| metrics.contains(&d.metric)).map(|d| d.id.clone()).collect();
            let surprise = inbox_devs.iter().any(|d| metrics.contains(&d.metric) && d.z.abs() > 4.0);
            let novelty = (c.novelty + if surprise { 0.2 } else { 0.0 }).min(1.0);
            let escalate = hit.escalate.or(if action.is_hold() { None } else if novelty > 0.6 { Some(Tier::Large) } else if novelty >= 0.3 { Some(Tier::Small) } else { None });
            if !action.is_hold() {
                acted_metrics.extend(metrics.iter().cloned());
            }
            match escalate {
                Some(Tier::Human) => {
                    let r = format!("{} Escalated to the accountable human.", legible(&hit));
                    let mut d = self.draft(td, c, m, action, hit.confidence, Tier::Human, r, Some(&hit), info, novelty);
                    d.rung_at_time = d.rung_at_time.min(Rung::ActWithApproval);
                    drafts.push(d);
                }
                Some(t @ (Tier::Small | Tier::Large)) => escalations.push(Escalation { class: c.id.clone(), hit, action, tier: t, novelty, info }),
                _ => {
                    if !action.is_hold() || c.record_holds {
                        let r = legible(&hit);
                        drafts.push(self.draft(td, c, m, action, hit.confidence, Tier::Rule, r, Some(&hit), info, novelty));
                    }
                }
            }
        }

        // ---- one batched LLM call for escalations
        let mut rationale = String::new();
        let mut dismiss_llm: BTreeMap<String, String> = BTreeMap::new();
        if !escalations.is_empty() {
            let tier = if escalations.iter().any(|e| e.tier == Tier::Large) { "large" } else { "small" };
            let items: Vec<Value> = escalations.iter().map(|e| {
                let c = &self.spec.classes[&e.class];
                json!({"class": e.class, "rule_row": e.hit.row + 1, "when": e.hit.when, "then": e.hit.then, "rule_action": describe_action(&e.action),
                       "bindings": e.hit.bindings, "rule_confidence": e.hit.confidence, "stakes_eur": c.stakes_eur, "novelty": e.novelty,
                       "reversibility": c.reversibility, "kind": c.kind, "assumptions": c.assumptions, "forecast": c.forecast,
                       "alternatives": c.rows.iter().map(|r| r.then.clone()).collect::<Vec<_>>()})
            }).collect();
            let assumptions: Vec<Value> = self.st.assumptions.items.iter()
                .filter(|a| a.module.as_deref() == Some(&m.id) || classes.iter().any(|c| c.assumptions.contains(&a.id)))
                .map(|a| json!({"id": a.id, "text": a.text, "confidence": a.confidence, "status": a.status})).collect();
            let inbox_v: Vec<Value> = inbox_devs.iter().map(|d| json!({"deviation": d.id, "metric": d.metric, "z": d.z, "interpretation": d.interpretation}))
                .chain(inbox.iter().map(|x| json!({"message": x.id, "from": x.from, "subject": x.subject}))).collect();
            let tpl = m.prompts.get("think").cloned().unwrap_or_else(|| format!(
                "You are the {} module of {{{{company}}}}. Decide each escalated item. Separate facts from inferences, give at least two hypotheses, name load-bearing assumptions, a reference class, second-order effects and what would falsify your choice.", m.name));
            let system = render_prompt(&tpl, &[("company", self.spec.company.name.clone()), ("module", m.name.clone()), ("day", day.to_string()),
                                              ("inbox", Value::Array(inbox_v.clone()).to_string()), ("kpis", json!(run.inputs.metrics).to_string()),
                                              ("assumptions", Value::Array(assumptions.clone()).to_string())]);
            let user = format!("Day {day} ({}). Escalated items:\n{}\nInbox:\n{}\nMetrics: {}\nReturn one decision per class. `action` uses decision-table `then` syntax or `hold`.",
                               td.rec.date, Value::Array(items.clone()), Value::Array(inbox_v.clone()), json!(run.inputs.metrics));
            let req = LlmRequest {
                tier: tier.into(), purpose: "module_think".into(), module: Some(m.id.clone()), system, user: user.clone(),
                schema_name: "module_decisions".into(), schema: Some(governance::think_schema()),
                context: json!({"module": m.id, "day": day, "items": items, "inbox": inbox_v, "metrics": run.inputs.metrics, "assumptions": assumptions}),
                day,
            };
            let r = self.llm.call(&req);
            run.think.prompt_ref = Some(if m.prompts.contains_key("think") { format!("modules/{}/prompts/think.md", m.id) } else { "engine:default_think".into() });
            run.think.llm_request = Some(user);
            run.think.llm_response = r.raw.clone();
            run.think.llm_model = Some(r.record.model.clone()).filter(|s| !s.is_empty());
            run.think.fallback = r.record.fallback.clone();
            run.cost_usd += r.record.cost_usd;
            run.tokens += r.record.tokens_in + r.record.tokens_out;
            let used_tier = match r.record.tier.as_str() { "large" => Tier::Large, "small" => Tier::Small, "rule" => Tier::Rule, _ => if tier == "large" { Tier::Large } else { Tier::Small } };
            run.think.llm = Some(r.record.clone());
            match r.value {
                Some(v) => {
                    rationale = v["rationale"].as_str().unwrap_or("").to_string();
                    for x in v["dismiss"].as_array().cloned().unwrap_or_default() {
                        if let (Some(a), Some(reason)) = (x["alert"].as_str(), x["reason"].as_str()) {
                            dismiss_llm.insert(a.into(), reason.into());
                        }
                    }
                    for text in v["anomalies"].as_array().cloned().unwrap_or_default().iter().filter_map(|t| t.as_str()) {
                        let a = Anomaly { id: self.st.rt.ids.next("AN"), day, by: m.id.clone(), text: text.into(), provenance: Provenance::Internal, cluster: None, tags: vec![] };
                        run.outputs.anomalies.push(a.id.clone());
                        td.rec.anomalies.push(a);
                    }
                    let answers = v["decisions"].as_array().cloned().unwrap_or_default();
                    for e in &escalations {
                        let c = self.spec.classes[&e.class].clone();
                        let ans = answers.iter().find(|a| a["class"].as_str() == Some(&e.class));
                        let Some(ans) = ans else {
                            run.think.rules_failed.push(format!("{}: LLM returned no decision; deferred", e.class));
                            continue;
                        };
                        let act_src = ans["action"].as_str().unwrap_or("hold");
                        let action = {
                            let ctx = MetricCtx::new(day, &kpis, &self.st.lattice, &self.st.rt.last_seen);
                            parse_then(act_src, &ctx)
                        };
                        let action = match action {
                            Ok(a) => a,
                            Err(err) => {
                                run.think.rules_failed.push(format!("{}: LLM action `{act_src}` unparseable ({err}); rule action used", e.class));
                                e.action.clone()
                            }
                        };
                        if action.is_hold() && !c.record_holds {
                            continue;
                        }
                        let conf = ans["confidence"].as_f64().unwrap_or(e.hit.confidence).clamp(0.0, 1.0);
                        let mut reasoning = ans["reasoning"].as_str().unwrap_or("").to_string();
                        let list = |k: &str| ans[k].as_array().map(|a| a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join("; ")).unwrap_or_default();
                        for (label, key) in [("Facts", "facts"), ("Inferences", "inferences"), ("Hypotheses", "hypotheses"), ("Second-order", "second_order_effects")] {
                            let l = list(key);
                            if !l.is_empty() { reasoning.push_str(&format!("\n{label}: {l}")); }
                        }
                        if let Some(f) = ans["falsification"].as_str().filter(|s| !s.is_empty()) { reasoning.push_str(&format!("\nFalsified if: {f}")); }
                        if let Some(f) = ans["reference_class"].as_str().filter(|s| !s.is_empty()) { reasoning.push_str(&format!("\nReference class: {f}")); }
                        reasoning.push_str(&format!("\n(Escalated from {}.)", legible(&e.hit)));
                        let mut d = self.draft(td, &c, m, action, conf, used_tier, reasoning, Some(&e.hit), e.info.clone(), e.novelty);
                        for a in ans["assumptions"].as_array().cloned().unwrap_or_default().iter().filter_map(|x| x.as_str()) {
                            if self.st.assumptions.get(a).is_some() && !d.assumptions.contains(&a.to_string()) { d.assumptions.push(a.into()); }
                        }
                        if let Some(x) = ans["expected"].as_str().filter(|s| !s.is_empty()) { d.expected = x.into(); }
                        if let Some(alts) = ans["alternatives"].as_array() {
                            let v: Vec<String> = alts.iter().filter_map(|x| x.as_str().map(String::from)).collect();
                            if !v.is_empty() { d.alternatives = v; }
                        }
                        let el = &mut d.action_elements;
                        if el.hypothesis.is_none() { el.hypothesis = ans["hypothesis"].as_str().filter(|s| !s.is_empty()).map(String::from); }
                        if el.learning_metric.is_none() { el.learning_metric = ans["learning_metric"].as_str().filter(|s| !s.is_empty()).map(String::from); }
                        if el.decision_point.is_none() { el.decision_point = ans["decision_point"].as_str().filter(|s| !s.is_empty()).map(String::from); }
                        drafts.push(d);
                    }
                }
                None => {
                    // budget exhausted: fall back to the rule result, visibly
                    for e in &escalations {
                        if e.action.is_hold() {
                            run.think.rules_failed.push(format!("{}: deferred to tomorrow (LLM budget exhausted)", e.class));
                            continue;
                        }
                        let c = self.spec.classes[&e.class].clone();
                        let r = format!("{} LLM budget exhausted: rule applied with reduced confidence.", legible(&e.hit));
                        drafts.push(self.draft(td, &c, m, e.action.clone(), e.hit.confidence * 0.8, Tier::Rule, r, Some(&e.hit), e.info.clone(), e.novelty));
                    }
                }
            }
        }

        // ---- explicit disposition of every alert in the inbox: routed today and
        // over-budget alerts queued earlier (§13.15). Nothing is silently dropped.
        let mut to_dispose: Vec<(Id, bool)> = run.inputs.deviations.iter().map(|i| (i.clone(), false)).collect();
        to_dispose.extend(run.inputs.queued_alerts.iter().map(|i| (i.clone(), true)));
        for (id, queued) in to_dispose {
            let dv = if queued { self.st.rt.open_devs.remove(&id) } else { td.rec.deviations.iter().find(|d| d.id == id).cloned() };
            let Some(mut dv) = dv else { continue };
            if dv.disposition.is_some() { continue; }
            let acted = drafts.iter().find(|d| d.information.contains(&id))
                .or_else(|| drafts.iter().find(|_| acted_metrics.contains(&dv.metric)));
            let disp = match acted {
                Some(d) => AlertDisposition { day, by: m.id.clone(), action: "acted".into(), reason: format!("covered by {}", d.id), decision: Some(d.id.clone()) },
                None => AlertDisposition { day, by: m.id.clone(), action: "dismissed".into(),
                    reason: dismiss_llm.get(&id).cloned().unwrap_or_else(|| format!("no decision rule fires on {}; within tolerance", dv.metric)), decision: None },
            };
            if let Some(t) = self.st.history.devs.iter_mut().find(|t| t.id == id) {
                if disp.action == "acted" { t.acted_day = Some(day) } else { t.dismissed_day = Some(day) }
                if queued {
                    td.agg.ack_latency_days = Some(td.agg.ack_latency_days.unwrap_or(0.0).max((day - t.day) as f64));
                }
            }
            if disp.action == "dismissed" { run.outputs.dismissed_alerts.push(id.clone()); }
            dv.disposition = Some(disp);
            if queued {
                td.rec.deviation_updates.push(dv);
            } else if let Some(x) = td.rec.deviations.iter_mut().find(|d| d.id == id) {
                // several modules may receive the same flag; the first disposition is recorded
                *x = dv;
            }
        }

        // ---- trace + outputs
        run.think.tier = Some(drafts.iter().map(|d| d.tier).max().unwrap_or(if run.think.llm.is_some() { Tier::Small } else { Tier::Rule }));
        if rationale.is_empty() {
            rationale = if run.think.rules_fired.is_empty() { "No decision table matched.".into() }
                else { run.think.rules_fired.iter().map(legible).collect::<Vec<_>>().join(" ") };
        }
        run.think.rationale = rationale;
        run.outputs.decisions = drafts.iter().map(|d| d.id.clone()).collect();
        run.duration_ms = t0.elapsed().as_millis() as u64;
        td.proposals.extend(drafts);
        run
    }

    /// Build a proposed ledger entry (the seven fields + extensions).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draft(&mut self, td: &Today, c: &DecisionClass, m: &ModuleSpec, action: ActionSpec, confidence: f64, tier: Tier,
                        reasoning: String, hit: Option<&RuleHit>, mut info: Vec<Id>, novelty: f64) -> Decision {
        let day = td.day;
        // information: cited deviations + the telemetry signals of the metrics the rule read
        if let Some(h) = hit {
            for k in h.bindings.keys() {
                let metric = k.split_once('(').map(|(_, r)| r.trim_end_matches(')')).unwrap_or(k);
                if let Some(s) = td.signal_of_metric.get(metric) {
                    if !info.contains(s) { info.push(s.clone()); }
                }
            }
        }
        let first_signal_day = info.iter().filter_map(|i| self.st.history.devs.iter().find(|t| &t.id == i).map(|t| t.day)).min();
        let rung = if self.enabled("autonomy", day) { self.st.autonomy.rung(&c.id) } else { c.default_rung };
        let alternatives = c.rows.iter().enumerate().filter(|(i, _)| hit.map(|h| h.row != *i).unwrap_or(true)).map(|(_, r)| r.then.clone()).collect();
        Decision {
            id: self.st.rt.ids.next("D"),
            class: c.id.clone(),
            module: m.id.clone(),
            what: format!("{}: {}", c.id, describe_action(&action)),
            when: day,
            who: m.id.clone(),
            information: info,
            assumptions: c.assumptions.clone(),
            expected: c.forecast.clone().unwrap_or_else(|| c.confirm_if.clone().map(|x| format!("{x} by day {}", day + c.check_after_days)).unwrap_or_default()),
            switches: DecisionSwitch { confirm_if: c.confirm_if.clone().unwrap_or_default(), stop_if: c.stop_if.clone().unwrap_or_default(), check_by: day + c.check_after_days.max(1) },
            action,
            kind: c.kind,
            reversibility: c.reversibility,
            impact: c.impact,
            path: c.path,
            stakes_eur: c.stakes_eur,
            novelty,
            confidence,
            forecasts: vec![],
            tier,
            rung_at_time: rung,
            reasoning,
            alternatives,
            premortem: None,
            premortem_started_ts: None,
            committed_ts: None,
            committed_day: None,
            challenges: vec![],
            gate: None,
            approvals: vec![],
            overrides: vec![],
            accountable: c.accountable.clone(),
            status: DecisionStatus::Proposed,
            outcome: None,
            action_elements: ActionElements {
                hypothesis: c.hypothesis.clone(),
                learning_metric: c.learning_metric.clone(),
                decision_point: c.decision_point.clone(),
                reversibility: c.requires_action_elements().then(|| format!("{} — {}", c.reversibility, describe_reversal(c.reversibility))),
            },
            commitment_point: c.commitment_point.clone(),
            informed: vec![],
            actions: vec![],
            first_signal_day,
            notes: vec![],
        }
    }
}

fn describe_reversal(r: Reversibility) -> &'static str {
    match r {
        Reversibility::Reversible => "can be undone at low cost",
        Reversibility::CostlyToReverse => "can be undone at a cost",
        Reversibility::Irreversible => "cannot be undone",
    }
}
