//! Tiny Games Oy: a toy company showing how a company plugs into the OTA engine.
//!
//! * `company/` — the human-readable spec (2 modules, 1 leader, 1 validator).
//! * [`ToyWorld`] — a seeded market model implementing [`ota_runtime::World`].
//! * [`TinyScripted`] — the deterministic responder used for history and offline runs.

use anyhow::Result;
use ota_core::*;
use ota_llm::{LlmRequest, Scripted};
use ota_runtime::{EventCtx, World, WorldDay};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

pub fn company_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("company")
}

/// Deterministic hash → [0, 1).
pub fn unit(s: &str) -> f64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    (h >> 11) as f64 / (1u64 << 53) as f64
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToyWorld {
    pub seed: u64,
    pub bid: f64,
    pub budget: f64,
    pub dau: f64,
    /// active shocks: (param, multiplier, start day, half-life days)
    pub shocks: Vec<(String, f64, Day, f64)>,
    pub tests: Vec<Day>,
    pub history: BTreeMap<String, f64>,
}

impl Default for ToyWorld {
    fn default() -> Self {
        ToyWorld { seed: 7, bid: 2.0, budget: 1500.0, dau: 45000.0, shocks: vec![], tests: vec![], history: BTreeMap::new() }
    }
}

impl ToyWorld {
    fn noise(&self, day: Day, k: &str) -> f64 {
        unit(&format!("{}:{day}:{k}", self.seed)) - 0.5
    }
    fn shock(&self, param: &str, day: Day) -> f64 {
        self.shocks.iter().filter(|s| s.0 == param && day >= s.2).map(|s| {
            let age = (day - s.2) as f64;
            1.0 + (s.1 - 1.0) * 0.5f64.powf(age / s.3)
        }).product()
    }
}

impl World for ToyWorld {
    fn generate_events(&mut self, day: Day, ctx: &mut EventCtx) -> Vec<EnvEvent> {
        if ctx.is_live() {
            let req = LlmRequest {
                tier: "env".into(), purpose: "env_events".into(), module: None,
                system: "You generate 0-2 plausible external events for a small mobile game studio's market.".into(),
                user: format!("Day {day} ({}). Recent events: {:?}", ctx.date, ctx.recent_events.iter().map(|e| &e.title).collect::<Vec<_>>()),
                schema_name: "events".into(), schema: Some(ota_tools::governance::env_events_schema()), context: json!({"day": day}), day,
            };
            let v = ctx.llm(&req).value.unwrap_or(json!({}));
            return v["events"].as_array().cloned().unwrap_or_default().iter().map(|e| EnvEvent {
                id: String::new(), day, kind: e["kind"].as_str().unwrap_or("news").into(), title: e["title"].as_str().unwrap_or("").into(),
                text: e["text"].as_str().unwrap_or("").into(), effects: serde_json::from_str(e["effects"].as_str().unwrap_or("{}")).unwrap_or(json!({})),
                tags: vec![], generated_by: "llm".into(),
            }).collect();
        }
        let mut ev = vec![];
        if day == 12 {
            ev.push(EnvEvent { id: String::new(), day, kind: "platform_policy".into(), title: "Platform privacy change raises CPIs".into(),
                               text: "Attribution signal loss across ad networks.".into(), effects: json!({"cpi": 1.45, "half_life_days": 25}),
                               tags: vec!["privacy".into()], generated_by: "storyline".into() });
        }
        if day == 20 {
            ev.push(EnvEvent { id: String::new(), day, kind: "competitor".into(), title: "Competitor clone launches".into(), text: String::new(),
                               effects: json!({"dau": 0.93, "half_life_days": 40}), tags: vec!["competitor".into()], generated_by: "storyline".into() });
        }
        if unit(&format!("news{day}")) < 0.15 {
            ev.push(EnvEvent { id: String::new(), day, kind: "news".into(), title: format!("Industry news #{day}"), text: String::new(),
                               effects: json!({}), tags: vec![], generated_by: "scripted".into() });
        }
        ev
    }

    fn advance(&mut self, day: Day, events: &[EnvEvent], _executed: &[Action]) -> Result<WorldDay> {
        for e in events {
            if let Some(m) = e.effects.as_object() {
                let hl = m.get("half_life_days").and_then(|v| v.as_f64()).unwrap_or(14.0);
                for (k, v) in m.iter().filter(|(k, _)| *k != "half_life_days") {
                    if let Some(x) = v.as_f64() { self.shocks.push((k.clone(), x, day, hl)); }
                }
            }
        }
        let weekday = 1.0 + 0.06 * (((day % 7) as f64) - 3.0) / 3.0;
        let cpi = (2.0 * self.shock("cpi", day) * (self.bid / 2.0).powf(0.35) * (1.0 + 0.08 * self.noise(day, "cpi"))).max(0.3);
        let util = (self.bid / (cpi * 0.95)).clamp(0.5, 1.0);
        let spend = self.budget * util;
        let installs_paid = spend / cpi;
        let organic = 1600.0 * weekday * (1.0 + 0.05 * self.noise(day, "org"));
        let arpu_d7 = 2.3 * (1.0 + 0.1 * self.noise(day, "arpu"));
        let roas = arpu_d7 / cpi;
        self.dau = 0.97 * self.dau + 0.6 * (installs_paid + organic);
        let dau_shock = self.shock("dau", day);
        let mut k = BTreeMap::new();
        k.insert("cpi".to_string(), cpi);
        k.insert("spend".to_string(), spend);
        k.insert("spend_util".to_string(), util);
        k.insert("bid".to_string(), self.bid);
        k.insert("installs".to_string(), installs_paid + organic);
        k.insert("roas_d7".to_string(), roas);
        k.insert("dau".to_string(), self.dau * weekday * dau_shock);
        k.insert("revenue".to_string(), self.dau * weekday * dau_shock * 0.13);
        let mut anomalies = vec![];
        if (24..=27).contains(&day) {
            for (i, by) in ["ceo", "ua-scientist", "analytics"].iter().enumerate() {
                if (day as usize - 24) == i || day == 27 {
                    anomalies.push(Anomaly { id: String::new(), day, by: by.to_string(), text: "Players report slow loading after the update".into(),
                                             provenance: Provenance::Internal, cluster: None, tags: vec!["loading".into()] });
                }
            }
        }
        let research = if day % 30 == 25 {
            vec![EnvEvent { id: format!("R{day}"), day, kind: "research".into(), title: "Peer studios report alert-budget retunes".into(),
                            text: "Peers cut routed alerts by 40 % with no missed incidents.".into(), effects: json!({}), tags: vec![], generated_by: "scripted".into() }]
        } else { vec![] };
        Ok(WorldDay { kpis: k, signals: vec![], anomalies, research, untraced_human_actions: 0, notes: vec![] })
    }

    fn apply_action(&mut self, _day: Day, a: &Action) -> Result<String> {
        let p = &a.params;
        Ok(match a.kind.as_str() {
            "adjust" => {
                let target = p["target"].as_str().unwrap_or("");
                let v = p["value"].as_f64().unwrap_or(1.0);
                let slot = match target { "bid" => &mut self.bid, "budget" => &mut self.budget, _ => return Ok(format!("unknown target {target}")) };
                let before = *slot;
                *slot = match p["op"].as_str().unwrap_or("set") { "*" => before * v, "+" => before + v, "-" => before - v, "/" => before / v, _ => v };
                *slot = slot.clamp(0.2, 50_000.0);
                self.history.insert(a.decision.clone(), before);
                format!("{target}: {before:.2} → {:.2}", *slot)
            }
            "set_budget" => {
                let before = self.budget;
                self.budget = p["daily"].as_f64().unwrap_or(self.budget).clamp(100.0, 20_000.0);
                format!("daily budget {before:.0} → {:.0}", self.budget)
            }
            "start_test" => {
                self.tests.push(a.executed_day);
                format!("network test started on {}", p["network"].as_str().unwrap_or("?"))
            }
            "revert" => "reverted to previous setting".into(),
            other => format!("{other}: recorded"),
        })
    }

    fn scripted(&self) -> Arc<dyn Scripted> {
        Arc::new(TinyScripted)
    }

    fn save(&self) -> Value {
        serde_json::to_value(self).unwrap_or_default()
    }

    fn load(&mut self, v: Value) -> Result<()> {
        *self = serde_json::from_value(v)?;
        Ok(())
    }
}

/// Deterministic stand-in for every LLM purpose.
pub struct TinyScripted;

impl Scripted for TinyScripted {
    fn respond(&self, req: &LlmRequest) -> Value {
        let c = &req.context;
        let h = |k: &str| unit(&format!("{}:{}:{k}", req.day, req.user.len()));
        match req.purpose.as_str() {
            "module_think" => {
                let decisions: Vec<Value> = c["items"].as_array().cloned().unwrap_or_default().iter().map(|it| json!({
                    "class": it["class"], "action": it["then"], "confidence": (it["rule_confidence"].as_f64().unwrap_or(0.5) * 0.95),
                    "reasoning": format!("Scripted review of {}: the rule's proposal stands.", it["class"].as_str().unwrap_or("")),
                    "facts": [format!("bindings {}", it["bindings"])], "inferences": ["the deviation is real, not noise"],
                    "hypotheses": ["market-wide cost shift", "creative fatigue"], "assumptions": it["assumptions"],
                    "reference_class": "past bid changes", "second_order_effects": ["organic halo changes"],
                    "falsification": "ROAS stays below target for 7 days", "expected": it["forecast"],
                })).collect();
                json!({"rationale": format!("Scripted think for {}: {} escalation(s).", c["module"].as_str().unwrap_or(""), decisions.len()),
                       "decisions": decisions, "dismiss": [], "anomalies": []})
            }
            "lattice_interpret" => json!({"interpretations": c["deviations"].as_array().cloned().unwrap_or_default().iter().map(|d| json!({
                "deviation": d["deviation"], "interpretation": format!("{} moved {:+.1}σ; check external events before acting.", d["metric"].as_str().unwrap_or(""), d["z"].as_f64().unwrap_or(0.0))
            })).collect::<Vec<_>>()}),
            "premortem" => json!({
                "failure_modes": [
                    {"text": "costs rise faster than revenue after the change", "p": 0.3, "severity": 0.6},
                    {"text": "attribution noise hides a real decline", "p": 0.2, "severity": 0.5},
                    {"text": "a competitor outbids us during the test window", "p": 0.15, "severity": 0.4}],
                "load_bearing": c["decision"]["assumptions"],
                "minutes": if h("pm") < 0.3 { 12.0 } else { 45.0 },
            }),
            "adversarial" => json!({"challenges": c["assumptions"].as_array().cloned().unwrap_or_default().iter().map(|a| json!({
                "target": a["id"], "argument": format!("Evidence for '{}' comes from a single attribution source.", a["text"].as_str().unwrap_or("")),
                "sources": ["network dashboard"], "strength": 0.5 + 0.3 * unit(a["id"].as_str().unwrap_or("")),
            })).collect::<Vec<_>>()}),
            "standin" => {
                let bias = &c["person"]["bias"];
                let diligence = bias["review_diligence"].as_f64().unwrap_or(0.7);
                let accept = bias["challenge_acceptance"].as_f64().unwrap_or(0.6);
                let strong = c["challenges"].as_array().map(|a| a.iter().any(|x| x["strength"].as_f64().unwrap_or(0.0) >= 0.7)).unwrap_or(false);
                let r = h(c["item"]["id"].as_str().unwrap_or(""));
                let decision = if strong && r < accept * 0.5 { "reject" } else if r < 0.08 { "amend" } else { "approve" };
                json!({"decision": decision, "reason": format!("{} reviewed {} with my own read of the numbers.", c["person"]["id"].as_str().unwrap_or(""), c["item"]["kind"].as_str().unwrap_or("")),
                       "amendments": if decision == "amend" { "halve the step size" } else { "" },
                       "minutes_spent": 2.0 + 20.0 * diligence * (0.5 + r), "confidence": 0.55 + 0.3 * r,
                       "manual_check": r < diligence * 0.6})
            }
            "brief" => {
                let items: Vec<Value> = c["items"].as_array().cloned().unwrap_or_default();
                json!({"headline": items.first().and_then(|x| x.as_str()).unwrap_or("Quiet day: no material deviations"), "items": items})
            }
            "meta_loop" => json!({"proposals": [{"title": "Retune alert budgets", "rationale": "Peer practice and our suppressed-alert share suggest it.", "tool": "lattice"}]}),
            "env_events" => json!({"events": []}),
            _ => json!({}),
        }
    }
}
