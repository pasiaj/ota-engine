//! Living Scenario Tree: a DAG of scenarios whose probabilities move with leading
//! indicators and assumption refutations. Governance tiers (§13.6):
//! `major` — AI proposes, leadership approves at the monthly meeting;
//! `minor` — AI adds it with a log entry, reviewed at the next review;
//! `urgent` — off-schedule approval request to a named decider.

use ota_core::*;
use ota_decide::{eval_bool, Context};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScenarioTree {
    pub nodes: Vec<ScenarioNode>,
}

impl ScenarioTree {
    pub fn get_mut(&mut self, id: &str) -> Option<&mut ScenarioNode> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    /// Weekly review: move probabilities with indicators, then renormalise siblings.
    pub fn review(&mut self, day: Day, ctx: &dyn Context, refuted: &[Id]) -> Vec<ScenarioNode> {
        let mut changed = vec![];
        for n in self.nodes.iter_mut().filter(|n| n.status == "approved" || n.status == "logged") {
            let before = n.p;
            let mut reasons = vec![];
            for ind in &n.leading_indicators {
                if eval_bool(&ind.expr, ctx).unwrap_or(false) {
                    if ind.effect == "refute" { n.p *= 0.85 } else { n.p = (n.p * 1.15).min(0.99) }
                    reasons.push(format!("{} {}", ind.effect, ind.expr));
                }
            }
            for a in n.assumptions.iter().filter(|a| refuted.contains(a)) {
                n.p *= 0.8;
                reasons.push(format!("assumption {a} refuted"));
            }
            if (n.p - before).abs() > 1e-9 {
                n.history.push(ConfidencePoint { day, confidence: n.p, reason: reasons.join("; ") });
            }
        }
        // renormalise groups of siblings that share the same parent set
        let mut groups: Vec<Vec<Id>> = vec![];
        for n in self.nodes.iter().filter(|n| n.status != "rejected" && n.status != "pending") {
            match groups.iter_mut().find(|g| self.nodes.iter().find(|x| x.id == g[0]).map(|x| x.parent == n.parent).unwrap_or(false)) {
                Some(g) => g.push(n.id.clone()),
                None => groups.push(vec![n.id.clone()]),
            }
        }
        for g in groups.into_iter().filter(|g| g.len() > 1) {
            let sum: f64 = g.iter().filter_map(|id| self.nodes.iter().find(|n| &n.id == id)).map(|n| n.p).sum();
            if sum <= 0.0 { continue; }
            for id in g {
                let n = self.get_mut(&id).unwrap();
                let np = n.p / sum;
                if (np - n.history.last().map(|h| h.confidence).unwrap_or(-1.0)).abs() > 0.005 && (np - n.p).abs() > 1e-9 {
                    n.history.push(ConfidencePoint { day, confidence: np, reason: "renormalised with siblings".into() });
                }
                n.p = np;
            }
        }
        for n in &self.nodes {
            if n.history.last().map(|h| h.day == day).unwrap_or(false) {
                changed.push(n.clone());
            }
        }
        changed
    }

    /// AI proposes a branch. Minor branches are added immediately with a log entry.
    pub fn propose(&mut self, mut n: ScenarioNode, day: Day) -> ScenarioNode {
        n.created = day;
        n.status = match n.tier { ScenarioTier::Minor => "logged", _ => "pending" }.into();
        n.history.push(ConfidencePoint { day, confidence: n.p, reason: format!("proposed ({})", n.tier) });
        self.nodes.push(n.clone());
        n
    }

    pub fn decide(&mut self, id: &str, approve: bool, by: &str, day: Day) -> Option<ScenarioNode> {
        let n = self.get_mut(id)?;
        n.status = if approve { "approved" } else { "rejected" }.into();
        n.approved_by = Some(by.to_string());
        n.history.push(ConfidencePoint { day, confidence: n.p, reason: format!("{} by {by}", n.status) });
        Some(n.clone())
    }

    pub fn pending(&self, tier: ScenarioTier) -> Vec<&ScenarioNode> {
        self.nodes.iter().filter(|n| n.status == "pending" && n.tier == tier).collect()
    }
}
