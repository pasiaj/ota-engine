//! Autonomy Ladder (§13.4): Path 1 classes start at act_then_review without earning it;
//! Path 2 authority is earned. Reviews are monthly for Path 2, quarterly for Path 1, and
//! off-cycle when thresholds are crossed. Evidence = Brier + agreement rate with humans.
//! Every change is a proposal the accountable leader approves; changes are reversible.

use ota_core::*;
use ota_spec::DecisionClass;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const PROMOTE_BRIER: f64 = 0.18;
pub const DEMOTE_BRIER: f64 = 0.30;
pub const PROMOTE_AGREEMENT: f64 = 0.8;
pub const DEMOTE_AGREEMENT: f64 = 0.5;
pub const MIN_N: usize = 6;
pub const OFF_CYCLE_COOLDOWN_DAYS: Day = 28;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Autonomy {
    pub rungs: BTreeMap<String, Rung>,
    pub history: Vec<AutonomyChange>,
    pub last_review: BTreeMap<String, Day>,
}

impl Autonomy {
    pub fn init(&mut self, classes: &BTreeMap<String, DecisionClass>) {
        for (id, c) in classes {
            self.rungs.entry(id.clone()).or_insert(c.default_rung);
        }
    }

    pub fn rung(&self, class: &str) -> Rung {
        self.rungs.get(class).copied().unwrap_or(Rung::Recommend)
    }

    /// Highest rung a class may reach.
    pub fn ceiling(c: &DecisionClass) -> Rung {
        match (c.path, c.reversibility) {
            (_, Reversibility::Irreversible) => Rung::ActWithApproval,
            (DecisionPath::Path1, _) => Rung::Autonomous,
            (DecisionPath::Path2, _) => Rung::ActThenReview,
        }
    }

    /// Is a scheduled review due for this class today?
    /// `month_start` / `quarter_start` come from the calendar of the simulation.
    pub fn review_due(&self, c: &DecisionClass, month_start: bool, quarter_start: bool) -> Option<&'static str> {
        match c.path {
            DecisionPath::Path2 if month_start => Some("monthly_review"),
            DecisionPath::Path1 if quarter_start => Some("quarterly_review"),
            _ => None,
        }
    }

    /// Decide whether to propose a change. `scheduled` is None for an off-cycle check
    /// (only demotions fire off-cycle).
    pub fn evaluate(&mut self, c: &DecisionClass, day: Day, ev: AutonomyEvidence, scheduled: Option<&str>, ids: &mut IdGen)
        -> Option<AutonomyChange> {
        if self.history.iter().any(|h| h.class == c.id && h.status == "proposed") {
            return None;
        }
        // off-cycle checks react to a threshold being crossed, not to the same evidence
        // again: no further off-cycle change within 28 days of an approved one
        if scheduled.is_none() && self.history.iter().any(|h| h.class == c.id && h.status == "approved" && h.day + OFF_CYCLE_COOLDOWN_DAYS > day) {
            return None;
        }
        let cur = self.rung(&c.id);
        let bad = ev.n >= MIN_N.min(4) && (ev.brier.map(|b| b > DEMOTE_BRIER).unwrap_or(false)
            || ev.agreement_rate.map(|a| a < DEMOTE_AGREEMENT).unwrap_or(false));
        let good = ev.n >= MIN_N && ev.brier.map(|b| b <= PROMOTE_BRIER).unwrap_or(false)
            && ev.agreement_rate.map(|a| a >= PROMOTE_AGREEMENT).unwrap_or(true);
        if scheduled.is_some() {
            self.last_review.insert(c.id.clone(), day);
        }
        let to = if bad && cur > Rung::Inform {
            Rung::from_level(cur.level() - 1)
        } else if good && scheduled.is_some() && cur < Self::ceiling(c) && !c.human_only {
            Rung::from_level(cur.level() + 1)
        } else {
            return None;
        };
        let ch = AutonomyChange {
            id: ids.next("AU"), day, class: c.id.clone(), from: cur, to,
            evidence: AutonomyEvidence { threshold: if to < cur { DEMOTE_BRIER } else { PROMOTE_BRIER }, ..ev },
            trigger: scheduled.unwrap_or("off_cycle").to_string(), status: "proposed".into(),
            approved_by: None, source: None,
        };
        self.history.push(ch.clone());
        Some(ch)
    }

    pub fn resolve(&mut self, id: &str, approved: bool, by: &str, source: HumanSource) -> Option<AutonomyChange> {
        let ch = self.history.iter_mut().find(|h| h.id == id)?;
        ch.status = if approved { "approved" } else { "rejected" }.into();
        ch.approved_by = Some(by.to_string());
        ch.source = Some(source);
        if approved {
            self.rungs.insert(ch.class.clone(), ch.to);
        }
        Some(ch.clone())
    }
}
