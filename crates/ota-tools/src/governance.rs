//! Domain Expert Gate, Automatic Pre-Mortem and Adversarial Engine helpers (schemas,
//! scripted-friendly request context, and record builders).

use ota_core::*;
use ota_decide::{eval_bool, Context};
use ota_spec::{CompanySpec, DecisionClass};
use serde_json::{json, Value};

// ---------------------------------------------------------------- gate

/// Classify a decision. A validator persona defines the rules; the system applies them
/// automatically (§13.3). Red beats yellow beats green.
pub fn gate_classify(spec: &CompanySpec, class: &DecisionClass, ctx: &dyn Context) -> GateReview {
    let mut best = GateReview { color: GateColor::Green, rule: "no rule matched".into(), validator: class.validator.clone(),
                                reviewed_by: None, reviewed_day: None, verdict: None, recommendation: None };
    for p in spec.people.iter().filter(|p| !p.gate_rules.is_empty()) {
        for r in p.gate_rules.iter().filter(|r| r.covers(&class.id)) {
            if let Some(e) = &r.red_if {
                if eval_bool(e, ctx).unwrap_or(false) {
                    return GateReview { color: GateColor::Red, rule: format!("{}: red_if {e}", p.id), validator: Some(p.id.clone()), ..best };
                }
            }
            if let Some(e) = &r.yellow_if {
                if best.color == GateColor::Green && eval_bool(e, ctx).unwrap_or(false) {
                    best.color = GateColor::Yellow;
                    best.rule = format!("{}: yellow_if {e}", p.id);
                    best.validator = Some(p.id.clone());
                }
            }
        }
    }
    best
}

// ---------------------------------------------------------------- pre-mortem

pub fn premortem_needed(stakes_eur: f64, novelty: f64, threshold: f64) -> bool {
    stakes_eur * novelty >= threshold
}

pub fn premortem_schema() -> Value {
    json!({"type":"object","properties":{
        "failure_modes":{"type":"array","items":{"type":"object","properties":{
            "text":{"type":"string"},"p":{"type":"number"},"severity":{"type":"number"}}}},
        "load_bearing":{"type":"array","items":{"type":"string"}},
        "minutes":{"type":"number"}}})
}

pub fn premortem_from(v: &Value, id: Id, d: &Decision, started_ts: Ts, tier: Tier) -> PreMortem {
    let fm = v["failure_modes"].as_array().cloned().unwrap_or_default().iter().map(|m| FailureModeItem {
        text: m["text"].as_str().unwrap_or("").to_string(),
        p: m["p"].as_f64().unwrap_or(0.2).clamp(0.0, 1.0),
        severity: m["severity"].as_f64().unwrap_or(0.5).clamp(0.0, 1.0),
    }).filter(|m| !m.text.is_empty()).collect();
    let lb = v["load_bearing"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
        .unwrap_or_else(|| d.assumptions.clone());
    PreMortem { id, decision: d.id.clone(), day: d.when, started_ts, failure_modes: fm, load_bearing: lb, skipped: false, skip_reason: None, tier }
}

// ---------------------------------------------------------------- adversarial

pub fn challenge_schema() -> Value {
    json!({"type":"object","properties":{
        "challenges":{"type":"array","items":{"type":"object","properties":{
            "target":{"type":"string"},"argument":{"type":"string"},
            "sources":{"type":"array","items":{"type":"string"}},"strength":{"type":"number"}}}}}})
}

/// Material assumptions of a decision: load-bearing or not yet confirmed.
pub fn material_assumptions<'a>(d: &Decision, cycle: &'a crate::assumptions::AssumptionCycle) -> Vec<&'a Assumption> {
    d.assumptions.iter().filter_map(|id| cycle.get(id))
        .filter(|a| a.load_bearing || a.confidence < 0.8)
        .collect()
}

pub fn challenges_from(v: &Value, d: &Decision, day: Day, ids: &mut IdGen) -> Vec<Challenge> {
    v["challenges"].as_array().cloned().unwrap_or_default().iter().filter_map(|c| {
        let arg = c["argument"].as_str()?.to_string();
        Some(Challenge {
            id: ids.next("CH"), day, target: c["target"].as_str().unwrap_or(&d.id).to_string(), decision: d.id.clone(),
            argument: arg, sources: c["sources"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default(),
            strength: c["strength"].as_f64().unwrap_or(0.5).clamp(0.0, 1.0), response: None, overridden_by: None, accepted: None,
        })
    }).collect()
}

/// Schema of the human stand-in response (§7).
pub fn standin_schema() -> Value {
    json!({"type":"object","properties":{
        "decision":{"type":"string","enum":["approve","reject","amend","defer"]},
        "reason":{"type":"string"},
        "amendments":{"type":"string"},
        "minutes_spent":{"type":"number"},
        "confidence":{"type":"number"},
        "manual_check":{"type":"boolean"}}})
}

/// Schema of the batched module-think response: one array of decisions (§5.6).
pub fn think_schema() -> Value {
    json!({"type":"object","properties":{
        "rationale":{"type":"string"},
        "decisions":{"type":"array","items":{"type":"object","properties":{
            "class":{"type":"string"},
            "action":{"type":"string","description":"`hold` or an action in decision-table `then` syntax"},
            "confidence":{"type":"number"},
            "reasoning":{"type":"string"},
            "facts":{"type":"array","items":{"type":"string"}},
            "inferences":{"type":"array","items":{"type":"string"}},
            "hypotheses":{"type":"array","items":{"type":"string"}},
            "assumptions":{"type":"array","items":{"type":"string"}},
            "reference_class":{"type":"string"},
            "second_order_effects":{"type":"array","items":{"type":"string"}},
            "falsification":{"type":"string"},
            "expected":{"type":"string"},
            "alternatives":{"type":"array","items":{"type":"string"}},
            "hypothesis":{"type":"string"},
            "learning_metric":{"type":"string"},
            "decision_point":{"type":"string"}}}},
        "dismiss":{"type":"array","items":{"type":"object","properties":{
            "alert":{"type":"string"},"reason":{"type":"string"}}}},
        "anomalies":{"type":"array","items":{"type":"string"}}}})
}

pub fn interpret_schema() -> Value {
    json!({"type":"object","properties":{"interpretations":{"type":"array","items":{"type":"object","properties":{
        "deviation":{"type":"string"},"interpretation":{"type":"string"}}}}}})
}

pub fn brief_schema() -> Value {
    json!({"type":"object","properties":{"headline":{"type":"string"},"items":{"type":"array","items":{"type":"string"}}}})
}

pub fn env_events_schema() -> Value {
    json!({"type":"object","properties":{"events":{"type":"array","items":{"type":"object","properties":{
        "kind":{"type":"string"},"title":{"type":"string"},"text":{"type":"string"},
        "effects":{"type":"string","description":"JSON object of parameter changes"},
        "tags":{"type":"array","items":{"type":"string"}}}}}}})
}

pub fn meta_loop_schema() -> Value {
    json!({"type":"object","properties":{"proposals":{"type":"array","items":{"type":"object","properties":{
        "title":{"type":"string"},"rationale":{"type":"string"},"tool":{"type":"string"}}}}}})
}
