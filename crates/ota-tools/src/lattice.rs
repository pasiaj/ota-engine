//! Reality Lattice (six levels, §13.8):
//! L1 coverage · L2 baselines · L3 detection · L4 interpretation · L5 routing with an
//! alert budget · L6 the decider (human, or AI with earned autonomy).
//!
//! The lattice also keeps the metric history that expressions (`z()`, `trend()`, ...)
//! read, so it is always maintained even before the tool is "enabled"; only
//! detection/routing are gated by the layered rollout.

use ota_core::*;
use ota_decide::Context;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const BASELINE_DAYS: usize = 28;
pub const HISTORY_KEEP: usize = 400;
pub const Z_THRESHOLD: f64 = 2.0;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Lattice {
    /// metric → (day, value), ascending by day.
    pub history: BTreeMap<String, Vec<(Day, f64)>>,
    /// Weekday index (0 = Mon) of day 1, for weekday seasonality.
    pub day1_weekday: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Baseline {
    pub mean: f64,
    pub sd: f64,
    pub n: usize,
    /// Multiplicative weekday factor applied to `mean` for the evaluated day.
    pub weekday_factor: f64,
}

impl Lattice {
    pub fn record(&mut self, day: Day, kpis: &BTreeMap<String, f64>) {
        for (k, v) in kpis {
            if !v.is_finite() {
                continue;
            }
            let h = self.history.entry(k.clone()).or_default();
            if h.last().map(|x| x.0) == Some(day) {
                h.pop();
            }
            h.push((day, *v));
            if h.len() > HISTORY_KEEP {
                h.drain(..h.len() - HISTORY_KEEP);
            }
        }
    }

    pub fn value(&self, metric: &str, day: Day) -> Option<f64> {
        self.history.get(metric)?.iter().rev().find(|x| x.0 == day).map(|x| x.1)
    }

    pub fn latest(&self, metric: &str) -> Option<(Day, f64)> {
        self.history.get(metric)?.last().copied()
    }

    fn before(&self, metric: &str, day: Day, n: usize) -> Vec<(Day, f64)> {
        let Some(h) = self.history.get(metric) else { return vec![] };
        let v: Vec<(Day, f64)> = h.iter().filter(|x| x.0 < day).copied().collect();
        v[v.len().saturating_sub(n)..].to_vec()
    }

    /// L2: trailing 28-day baseline (excluding `day`) with weekday seasonality.
    pub fn baseline(&self, metric: &str, day: Day) -> Option<Baseline> {
        let w = self.before(metric, day, BASELINE_DAYS);
        if w.len() < 7 {
            return None;
        }
        let n = w.len() as f64;
        let mean = w.iter().map(|x| x.1).sum::<f64>() / n;
        let wd = weekday_of(day, self.day1_weekday);
        let same: Vec<f64> = w.iter().filter(|x| weekday_of(x.0, self.day1_weekday) == wd).map(|x| x.1).collect();
        let weekday_factor = if same.len() >= 3 && mean.abs() > 1e-9 {
            (same.iter().sum::<f64>() / same.len() as f64 / mean).clamp(0.5, 2.0)
        } else {
            1.0
        };
        // residual sd after weekday adjustment
        let resid: Vec<f64> = w.iter().map(|x| {
            let f = if mean.abs() > 1e-9 && same.len() >= 3 { self.weekday_factor_for(&w, x.0, mean) } else { 1.0 };
            x.1 - mean * f
        }).collect();
        let var = resid.iter().map(|r| r * r).sum::<f64>() / (n - 1.0).max(1.0);
        let sd = var.sqrt().max(mean.abs() * 0.01).max(1e-9);
        Some(Baseline { mean, sd, n: w.len(), weekday_factor })
    }

    fn weekday_factor_for(&self, w: &[(Day, f64)], d: Day, mean: f64) -> f64 {
        let wd = weekday_of(d, self.day1_weekday);
        let same: Vec<f64> = w.iter().filter(|x| weekday_of(x.0, self.day1_weekday) == wd).map(|x| x.1).collect();
        if same.len() >= 3 { (same.iter().sum::<f64>() / same.len() as f64 / mean).clamp(0.5, 2.0) } else { 1.0 }
    }

    pub fn z(&self, metric: &str, day: Day) -> Option<f64> {
        let v = self.value(metric, day)?;
        let b = self.baseline(metric, day)?;
        Some((v - b.mean * b.weekday_factor) / b.sd)
    }

    /// Relative least-squares slope per day over the last `days` values up to `day`.
    pub fn trend(&self, metric: &str, day: Day, days: u32) -> Option<f64> {
        let mut w = self.before(metric, day + 1, days as usize);
        w.retain(|x| x.0 <= day);
        if w.len() < 3 {
            return None;
        }
        let n = w.len() as f64;
        let mx = w.iter().map(|x| x.0 as f64).sum::<f64>() / n;
        let my = w.iter().map(|x| x.1).sum::<f64>() / n;
        let sxy: f64 = w.iter().map(|x| (x.0 as f64 - mx) * (x.1 - my)).sum();
        let sxx: f64 = w.iter().map(|x| (x.0 as f64 - mx).powi(2)).sum();
        if sxx == 0.0 || my.abs() < 1e-12 {
            return Some(0.0);
        }
        Some(sxy / sxx / my.abs())
    }

    pub fn mean(&self, metric: &str, day: Day, days: u32) -> Option<f64> {
        let w = self.before(metric, day + 1, days as usize);
        (!w.is_empty()).then(|| w.iter().map(|x| x.1).sum::<f64>() / w.len() as f64)
    }

    pub fn prev(&self, metric: &str, day: Day, days: u32) -> Option<f64> {
        self.value(metric, day.checked_sub(days)?)
    }

    /// L1: metrics expected (module inputs + KPIs) but missing today.
    pub fn coverage_gaps(&self, expected: &[String], today: &BTreeMap<String, f64>) -> Vec<String> {
        let mut v: Vec<String> = expected.iter().filter(|m| !today.contains_key(*m)).cloned().collect();
        v.sort();
        v.dedup();
        v
    }

    /// L3: z-score and trend-break detection for today's KPIs.
    pub fn detect(&self, day: Day, kpis: &BTreeMap<String, f64>, ids: &mut IdGen) -> Vec<Deviation> {
        let mut out = vec![];
        for (m, v) in kpis {
            let Some(b) = self.baseline(m, day) else { continue };
            let z = (v - b.mean * b.weekday_factor) / b.sd;
            if z.abs() >= Z_THRESHOLD {
                out.push(Deviation {
                    id: ids.next("DV"), day, metric: m.clone(), value: *v, baseline: b.mean * b.weekday_factor,
                    sigma: b.sd, z, level: 3, detector: "z".into(), interpretation: None, routed_to: vec![],
                    suppressed_by_budget: false, disposition: None,
                });
                continue;
            }
            // trend break: the last 7 days move against the 28-day trend by > 3 %/day
            if let (Some(t7), Some(t28)) = (self.trend(m, day, 7), self.trend(m, day, 28)) {
                if b.n >= 21 && (t7 - t28).abs() > 0.03 && t7.signum() != t28.signum() {
                    out.push(Deviation {
                        id: ids.next("DV"), day, metric: m.clone(), value: *v, baseline: b.mean, sigma: b.sd, z,
                        level: 3, detector: "trend_break".into(),
                        interpretation: None, routed_to: vec![], suppressed_by_budget: false, disposition: None,
                    });
                }
            }
        }
        out.sort_by(|a, b| b.z.abs().partial_cmp(&a.z.abs()).unwrap_or(std::cmp::Ordering::Equal));
        out
    }
}

/// L5: route deviations to recipients with a per-node daily alert budget. Over-budget
/// alerts are marked `suppressed_by_budget` and returned as a queue the recipient must
/// explicitly dismiss or act on (§13.15).
pub fn route(devs: &mut [Deviation], recipients: &dyn Fn(&str) -> Vec<NodeId>, budget: &dyn Fn(&str) -> usize)
    -> BTreeMap<NodeId, Vec<Id>> {
    let mut used: BTreeMap<NodeId, usize> = BTreeMap::new();
    let mut queued: BTreeMap<NodeId, Vec<Id>> = BTreeMap::new();
    for d in devs.iter_mut() {
        let rs = recipients(&d.metric);
        let mut delivered = false;
        for r in rs {
            let u = used.entry(r.clone()).or_insert(0);
            if *u < budget(&r) {
                *u += 1;
                d.routed_to.push(r);
                delivered = true;
            } else {
                queued.entry(r).or_default().push(d.id.clone());
            }
        }
        if !delivered {
            d.suppressed_by_budget = true;
        }
        if d.level < 5 && !d.routed_to.is_empty() {
            d.level = 5;
        }
    }
    queued
}

/// Expression context over today's KPIs, lattice history and "last seen" days.
pub struct MetricCtx<'a> {
    pub day: Day,
    pub kpis: &'a BTreeMap<String, f64>,
    pub lattice: &'a Lattice,
    /// name → last day seen (decision classes, action kinds, event kinds/tags).
    pub last_seen: &'a BTreeMap<String, Day>,
    /// Extra variables (e.g. decision attributes for gate rules).
    pub extra: BTreeMap<String, f64>,
}

impl<'a> MetricCtx<'a> {
    pub fn new(day: Day, kpis: &'a BTreeMap<String, f64>, lattice: &'a Lattice, last_seen: &'a BTreeMap<String, Day>) -> Self {
        MetricCtx { day, kpis, lattice, last_seen, extra: BTreeMap::new() }
    }
    pub fn with(mut self, k: &str, v: f64) -> Self {
        self.extra.insert(k.to_string(), v);
        self
    }
}

impl Context for MetricCtx<'_> {
    fn var(&self, name: &str) -> Option<f64> {
        self.extra.get(name).or_else(|| self.kpis.get(name)).copied()
            .or_else(|| self.lattice.latest(name).filter(|x| x.0 + 7 >= self.day).map(|x| x.1))
    }
    fn z(&self, m: &str) -> Option<f64> {
        self.lattice.z(m, self.day)
    }
    fn trend(&self, m: &str, d: u32) -> Option<f64> {
        self.lattice.trend(m, self.day, d)
    }
    fn days_since(&self, n: &str) -> Option<f64> {
        // never seen → a large number, so "days_since(x) > 21" is true before the first x
        Some(self.last_seen.get(n).map(|d| self.day.saturating_sub(*d) as f64).unwrap_or(9999.0))
    }
    fn mean(&self, m: &str, d: u32) -> Option<f64> {
        self.lattice.mean(m, self.day, d)
    }
    fn prev(&self, m: &str, d: u32) -> Option<f64> {
        self.lattice.prev(m, self.day, d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn baseline_z_trend_and_routing() {
        let mut l = Lattice::default();
        for d in 1..=30 {
            let mut k = BTreeMap::new();
            k.insert("x".to_string(), 100.0 + (d % 3) as f64);
            k.insert("up".to_string(), 100.0 + d as f64 * 2.0);
            l.record(d, &k);
        }
        let mut k = BTreeMap::new();
        k.insert("x".to_string(), 140.0);
        l.record(31, &k);
        let z = l.z("x", 31).unwrap();
        assert!(z > 5.0, "z = {z}");
        assert!(l.trend("up", 30, 7).unwrap() > 0.0);
        let mut ids = IdGen::default();
        ids.set_day(31);
        let mut devs = l.detect(31, &k, &mut ids);
        assert_eq!(devs.len(), 1);
        let q = route(&mut devs, &|_| vec!["a".into()], &|_| 0);
        assert!(devs[0].suppressed_by_budget);
        assert_eq!(q["a"].len(), 1);
        let last = BTreeMap::new();
        let ctx = MetricCtx::new(31, &k, &l, &last);
        assert_eq!(ctx.days_since("never"), Some(9999.0));
        assert!(ota_decide::eval_bool("z(x) > 3 && up > 100", &ctx).unwrap());
    }
}
