//! Assumption Cycle: indicators confirm or refute; silence ages assumptions to `stale`
//! after 60 days (the graveyard), and a human must archive or renew them.

use ota_core::*;
use ota_decide::{eval_bool, Context};
use ota_spec::AssumptionSpec;
use serde::{Deserialize, Serialize};

pub const STALE_AFTER_DAYS: Day = 60;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AssumptionCycle {
    pub items: Vec<Assumption>,
}

impl AssumptionCycle {
    pub fn from_specs(specs: &[AssumptionSpec], day: Day) -> Self {
        AssumptionCycle {
            items: specs.iter().map(|s| Assumption {
                id: s.id.clone(), text: s.text.clone(), owner: s.owner.clone(), module: s.module.clone(),
                confidence: s.confidence, load_bearing: s.load_bearing, created: day, last_confirmed: day,
                status: AssumptionStatus::Active, indicators: s.indicators.clone(), depends: s.depends.clone(),
                history: vec![ConfidencePoint { day, confidence: s.confidence, reason: "seeded".into() }],
                stale_since: None,
            }).collect(),
        }
    }

    /// Add assumptions that appeared in the spec since the state was saved.
    pub fn sync_specs(&mut self, specs: &[AssumptionSpec], day: Day) {
        for s in specs {
            if !self.items.iter().any(|a| a.id == s.id) {
                self.items.extend(AssumptionCycle::from_specs(std::slice::from_ref(s), day).items);
            }
        }
    }

    pub fn get(&self, id: &str) -> Option<&Assumption> {
        self.items.iter().find(|a| a.id == id)
    }
    pub fn get_mut(&mut self, id: &str) -> Option<&mut Assumption> {
        self.items.iter_mut().find(|a| a.id == id)
    }

    /// Evaluate all live assumptions. Returns the changes.
    pub fn evaluate(&mut self, day: Day, ctx: &dyn Context) -> Vec<AssumptionUpdate> {
        let mut out = vec![];
        for a in self.items.iter_mut() {
            if matches!(a.status, AssumptionStatus::Archived | AssumptionStatus::Refuted) {
                continue;
            }
            let before = (a.confidence, a.status);
            let (mut confirmed, mut refuted) = (vec![], vec![]);
            for ind in &a.indicators {
                if eval_bool(&ind.expr, ctx).unwrap_or(false) {
                    if ind.effect == "refute" { refuted.push(ind.expr.clone()) } else { confirmed.push(ind.expr.clone()) }
                }
            }
            let reason;
            if !refuted.is_empty() {
                a.confidence = (a.confidence * 0.8).max(0.01);
                reason = format!("refuted by {}", refuted.join(", "));
                a.status = if a.confidence < 0.3 { AssumptionStatus::Refuted } else { AssumptionStatus::Active };
                a.last_confirmed = day; // it received signal
                a.stale_since = None;
            } else if !confirmed.is_empty() {
                a.confidence = (a.confidence + (1.0 - a.confidence) * 0.05).min(0.99);
                a.last_confirmed = day;
                a.stale_since = None;
                a.status = if a.confidence >= 0.8 { AssumptionStatus::Confirmed } else { AssumptionStatus::Active };
                reason = format!("confirmed by {}", confirmed.join(", "));
            } else if day.saturating_sub(a.last_confirmed) >= STALE_AFTER_DAYS && a.status != AssumptionStatus::Stale {
                a.status = AssumptionStatus::Stale;
                a.stale_since = Some(day);
                reason = format!("no signal for {} days", day - a.last_confirmed);
            } else {
                continue;
            }
            if (before.0 - a.confidence).abs() > 1e-9 || before.1 != a.status {
                // keep the history compact: one point per change of > 0.02 or of status
                if (before.0 - a.confidence).abs() > 0.02 || before.1 != a.status {
                    a.history.push(ConfidencePoint { day, confidence: a.confidence, reason: reason.clone() });
                }
                if before.1 != a.status || (before.0 - a.confidence).abs() > 0.02 {
                    out.push(AssumptionUpdate { assumption: a.id.clone(), from: before.0, to: a.confidence, status: a.status, reason });
                }
            }
        }
        out
    }

    pub fn renew(&mut self, id: &str, day: Day, by: &str) {
        if let Some(a) = self.get_mut(id) {
            a.status = AssumptionStatus::Active;
            a.last_confirmed = day;
            a.stale_since = None;
            a.history.push(ConfidencePoint { day, confidence: a.confidence, reason: format!("renewed by {by}") });
        }
    }

    pub fn archive(&mut self, id: &str, day: Day, by: &str) {
        if let Some(a) = self.get_mut(id) {
            a.status = AssumptionStatus::Archived;
            a.history.push(ConfidencePoint { day, confidence: a.confidence, reason: format!("archived by {by}") });
        }
    }

    /// The graveyard: stale assumptions nobody has archived or renewed.
    pub fn graveyard(&self) -> Vec<&Assumption> {
        self.items.iter().filter(|a| a.status == AssumptionStatus::Stale).collect()
    }
}
