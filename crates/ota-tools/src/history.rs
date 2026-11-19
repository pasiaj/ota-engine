//! Per-day aggregates the monitors (Sentinel, audit, grid, metrics) compute trends over.

use ota_core::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DayAgg {
    pub day: Day,
    pub decisions: usize,
    pub ai_mean_confidence: Option<f64>,
    pub external_share: Option<f64>,
    /// Overrides / accepted exceptions by kind (`challenge`, `gate`, `stop_signal`, ...).
    pub overrides: BTreeMap<String, usize>,
    pub challenges: usize,
    pub challenges_overridden: usize,
    pub human_rejections: usize,
    pub premortem_similarity: Option<f64>,
    pub premortems_run: usize,
    pub premortems_skipped: usize,
    /// Mean minutes between pre-mortem start and commit (decision laundering).
    pub laundering_gap_min: Option<f64>,
    pub switches_overdue: usize,
    pub switches_due: usize,
    pub switches_sealed_on_time: usize,
    pub stop_crossed_not_stopped: usize,
    pub stale_assumptions: usize,
    pub alerts_routed: usize,
    pub alerts_suppressed: usize,
    pub alerts_open: usize,
    pub ack_latency_days: Option<f64>,
    pub high_stakes_approvals: usize,
    pub fast_high_stakes_approvals: usize,
    pub copied_reasons: usize,
    pub human_actions: usize,
    pub manual_checks: usize,
    pub human_minutes: f64,
    pub real_human_actions: usize,
    pub no_accountable: usize,
    pub diffuse_approvals: usize,
    pub single_source_high_conf: usize,
    pub module_similarity: Option<f64>,
    pub untraced_human_actions: usize,
    pub double_loop_recurring: usize,
    pub bad_news_by_humans: usize,
    pub kpis_decaying: usize,
    pub coherence_gap: Option<f64>,
    pub missing_action_elements: usize,
}

/// Tracks each ≥2σ deviation until a decision cites it or it is dismissed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DevTrack {
    pub id: Id,
    pub day: Day,
    pub metric: String,
    pub z: f64,
    pub routed: bool,
    pub acted_day: Option<Day>,
    pub dismissed_day: Option<Day>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct History {
    pub days: Vec<DayAgg>,
    pub devs: Vec<DevTrack>,
}

impl History {
    pub fn push(&mut self, a: DayAgg) {
        self.days.retain(|d| d.day != a.day);
        self.days.push(a);
    }

    pub fn window(&self, from: Day, to: Day) -> impl Iterator<Item = &DayAgg> {
        self.days.iter().filter(move |d| d.day >= from && d.day <= to)
    }

    pub fn compact(&mut self, day: Day) {
        self.devs.retain(|d| d.day + 120 >= day);
    }

    pub fn sum(&self, from: Day, to: Day, f: impl Fn(&DayAgg) -> usize) -> usize {
        self.window(from, to).map(f).sum()
    }

    pub fn mean(&self, from: Day, to: Day, f: impl Fn(&DayAgg) -> Option<f64>) -> Option<f64> {
        let v: Vec<f64> = self.window(from, to).filter_map(f).collect();
        (!v.is_empty()).then(|| v.iter().sum::<f64>() / v.len() as f64)
    }
}

/// Lexical (word-set Jaccard) similarity, mean over all pairs. Used for pre-mortem
/// diversity (groupthink) and module output convergence (homogenisation).
pub fn mean_pairwise_similarity(texts: &[String]) -> Option<f64> {
    let sets: Vec<std::collections::BTreeSet<String>> = texts.iter().map(|t| {
        t.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| w.len() > 3).map(String::from).collect()
    }).filter(|s: &std::collections::BTreeSet<String>| !s.is_empty()).collect();
    if sets.len() < 2 {
        return None;
    }
    let (mut sum, mut n) = (0.0, 0);
    for i in 0..sets.len() {
        for j in i + 1..sets.len() {
            let inter = sets[i].intersection(&sets[j]).count() as f64;
            let uni = sets[i].union(&sets[j]).count() as f64;
            sum += inter / uni;
            n += 1;
        }
    }
    Some(sum / n as f64)
}
