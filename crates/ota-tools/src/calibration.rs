//! Calibration Tracker: Brier scores and reliability bins.
//! Individual human profiles are private (§13.5): only aggregates are public.

use ota_core::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Calibration {
    pub forecasts: Vec<Forecast>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Bin {
    pub p_lo: f64,
    pub p_hi: f64,
    pub n: usize,
    pub mean_p: f64,
    pub freq: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Profile {
    pub n: usize,
    pub brier: Option<f64>,
    pub mean_p: Option<f64>,
    pub base_rate: Option<f64>,
    pub bins: Vec<Bin>,
}

/// The public calibration view: aggregates and per-AI-module/class profiles only.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PublicCalibration {
    pub overall_ai: Profile,
    pub overall_human: Profile,
    pub by_module: BTreeMap<String, Profile>,
    pub by_class: BTreeMap<String, Profile>,
    pub open_forecasts: usize,
}

pub fn profile<'a>(it: impl Iterator<Item = &'a Forecast>) -> Profile {
    let res: Vec<&Forecast> = it.filter(|f| f.outcome.is_some()).collect();
    if res.is_empty() {
        return Profile::default();
    }
    let n = res.len();
    let brier = res.iter().map(|f| f.brier.unwrap_or_else(|| ota_core::brier(f.p, f.outcome.unwrap()))).sum::<f64>() / n as f64;
    let mean_p = res.iter().map(|f| f.p).sum::<f64>() / n as f64;
    let base = res.iter().filter(|f| f.outcome == Some(true)).count() as f64 / n as f64;
    let mut bins = vec![];
    for i in 0..5 {
        let (lo, hi) = (i as f64 * 0.2, (i + 1) as f64 * 0.2);
        let b: Vec<&&Forecast> = res.iter().filter(|f| f.p >= lo && (f.p < hi || (i == 4 && f.p <= 1.0))).collect();
        if b.is_empty() {
            continue;
        }
        bins.push(Bin {
            p_lo: lo, p_hi: hi, n: b.len(),
            mean_p: b.iter().map(|f| f.p).sum::<f64>() / b.len() as f64,
            freq: b.iter().filter(|f| f.outcome == Some(true)).count() as f64 / b.len() as f64,
        });
    }
    Profile { n, brier: Some(brier), mean_p: Some(mean_p), base_rate: Some(base), bins }
}

impl Calibration {
    pub fn add(&mut self, f: Forecast) {
        self.forecasts.push(f);
    }

    pub fn due(&self, day: Day) -> Vec<usize> {
        self.forecasts.iter().enumerate().filter(|(_, f)| f.outcome.is_none() && f.resolves_on <= day).map(|(i, _)| i).collect()
    }

    pub fn resolve(&mut self, idx: usize, outcome: bool) -> Forecast {
        let f = &mut self.forecasts[idx];
        f.outcome = Some(outcome);
        f.brier = Some(ota_core::brier(f.p, outcome));
        f.clone()
    }

    /// Brier over the decision class's resolved forecasts within the window.
    pub fn class_profile(&self, class: &str, since: Day, class_of: &dyn Fn(&Forecast) -> Option<String>) -> Profile {
        profile(self.forecasts.iter().filter(|f| !f.human && f.resolves_on >= since && class_of(f).as_deref() == Some(class)))
    }

    pub fn brier_window(&self, from: Day, to: Day, human: Option<bool>) -> Option<f64> {
        profile(self.forecasts.iter().filter(|f| f.resolves_on >= from && f.resolves_on <= to && human.map(|h| f.human == h).unwrap_or(true))).brier
    }

    pub fn mean_p_window(&self, from: Day, to: Day) -> Option<f64> {
        let v: Vec<f64> = self.forecasts.iter().filter(|f| f.day >= from && f.day <= to && !f.human).map(|f| f.p).collect();
        (!v.is_empty()).then(|| v.iter().sum::<f64>() / v.len() as f64)
    }

    pub fn public(&self, topic_module: &dyn Fn(&Forecast) -> (Option<String>, Option<String>)) -> PublicCalibration {
        let mut by_module: BTreeMap<String, Vec<&Forecast>> = BTreeMap::new();
        let mut by_class: BTreeMap<String, Vec<&Forecast>> = BTreeMap::new();
        for f in self.forecasts.iter().filter(|f| !f.human) {
            let (m, c) = topic_module(f);
            if let Some(m) = m { by_module.entry(m).or_default().push(f); }
            if let Some(c) = c { by_class.entry(c).or_default().push(f); }
        }
        PublicCalibration {
            overall_ai: profile(self.forecasts.iter().filter(|f| !f.human)),
            overall_human: profile(self.forecasts.iter().filter(|f| f.human)),
            by_module: by_module.into_iter().map(|(k, v)| (k, profile(v.into_iter()))).collect(),
            by_class: by_class.into_iter().map(|(k, v)| (k, profile(v.into_iter()))).collect(),
            open_forecasts: self.forecasts.iter().filter(|f| f.outcome.is_none()).count(),
        }
    }

    /// Private profile for one human: visible only to that person.
    pub fn person(&self, id: &str) -> Profile {
        profile(self.forecasts.iter().filter(|f| f.human && f.by == id))
    }

    /// Drop resolved forecasts older than `keep_days` to bound the state size.
    pub fn compact(&mut self, day: Day, keep_days: Day) {
        self.forecasts.retain(|f| f.outcome.is_none() || f.resolves_on + keep_days >= day);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn f(p: f64, o: bool, human: bool) -> Forecast {
        Forecast { id: "f".into(), day: 1, by: "x".into(), question: "q".into(), p, resolves_on: 2, outcome: Some(o),
                   brier: None, topic: "t".into(), horizon_days: 1, decision: None, resolve_if: None, human }
    }
    #[test]
    fn brier_profile() {
        let c = Calibration { forecasts: vec![f(0.9, true, false), f(0.9, false, false), f(0.2, false, true)] };
        let p = profile(c.forecasts.iter().filter(|f| !f.human));
        assert_eq!(p.n, 2);
        assert!((p.brier.unwrap() - (0.01 + 0.81) / 2.0).abs() < 1e-9);
        assert_eq!(c.person("x").n, 1);
    }
}
