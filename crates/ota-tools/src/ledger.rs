//! Decision Ledger: 7 fields + decision switches + outcome sealing + search.

use ota_core::*;
use ota_decide::{eval_bool, Context};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Ledger {
    pub decisions: BTreeMap<Id, Decision>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LedgerQuery {
    pub q: Option<String>,
    pub class: Option<String>,
    pub module: Option<String>,
    pub status: Option<String>,
    pub from: Option<Day>,
    pub to: Option<Day>,
    pub limit: Option<usize>,
}

impl Ledger {
    pub fn insert(&mut self, d: Decision) {
        self.decisions.insert(d.id.clone(), d);
    }
    pub fn get(&self, id: &str) -> Option<&Decision> {
        self.decisions.get(id)
    }
    pub fn get_mut(&mut self, id: &str) -> Option<&mut Decision> {
        self.decisions.get_mut(id)
    }

    /// Decisions whose switch check date has arrived and that are not yet sealed.
    pub fn due(&self, day: Day) -> Vec<Id> {
        self.decisions.values()
            .filter(|d| d.outcome.is_none() && d.switches.check_by <= day
                && matches!(d.status, DecisionStatus::Executing | DecisionStatus::Approved))
            .map(|d| d.id.clone()).collect()
    }

    /// Evaluate the switches and seal the outcome (*no decision without an outcome*).
    pub fn seal(&mut self, id: &str, day: Day, ctx: &dyn Context) -> Option<Outcome> {
        let d = self.decisions.get_mut(id)?;
        let confirm = eval_bool(&d.switches.confirm_if, ctx);
        let stop = eval_bool(&d.switches.stop_if, ctx);
        let confirm_hit = *confirm.as_ref().unwrap_or(&false);
        let stop_hit = *stop.as_ref().unwrap_or(&false);
        let success = match (confirm_hit, stop_hit) {
            (_, true) => Some(false),
            (true, false) => Some(true),
            _ => None,
        };
        let mut metrics = BTreeMap::new();
        for e in [&d.switches.confirm_if, &d.switches.stop_if] {
            if let Ok(p) = ota_decide::parse(e) {
                metrics.extend(ota_decide::bindings(&p, ctx));
            }
        }
        let actual = match success {
            Some(true) => format!("confirm signal hit: {}", d.switches.confirm_if),
            Some(false) => format!("stop signal hit: {}", d.switches.stop_if),
            None => {
                let mut s = "neither switch fired (inconclusive)".to_string();
                if let Err(e) = confirm { s.push_str(&format!("; confirm unevaluable: {e}")); }
                if let Err(e) = stop { s.push_str(&format!("; stop unevaluable: {e}")); }
                s
            }
        };
        let o = Outcome {
            sealed_day: day,
            deviation: match success { Some(true) => "as expected".into(), Some(false) => "worse than expected".into(), None => "unclear".into() },
            expected: d.expected.clone(),
            actual,
            success,
            confirm_hit,
            stop_hit,
            metrics,
        };
        d.outcome = Some(o.clone());
        d.status = DecisionStatus::Sealed;
        Some(o)
    }

    pub fn search(&self, q: &LedgerQuery) -> Vec<&Decision> {
        let needle = q.q.as_ref().map(|s| s.to_lowercase());
        let mut v: Vec<&Decision> = self.decisions.values().filter(|d| {
            q.class.as_ref().map(|c| &d.class == c || (c.ends_with('*') && d.class.starts_with(c.trim_end_matches('*')))).unwrap_or(true)
                && q.module.as_ref().map(|m| &d.module == m).unwrap_or(true)
                && q.status.as_ref().map(|s| d.status.as_str() == s).unwrap_or(true)
                && q.from.map(|f| d.when >= f).unwrap_or(true)
                && q.to.map(|t| d.when <= t).unwrap_or(true)
                && needle.as_ref().map(|n| {
                    d.id.to_lowercase().contains(n) || d.what.to_lowercase().contains(n)
                        || d.reasoning.to_lowercase().contains(n) || d.class.to_lowercase().contains(n)
                }).unwrap_or(true)
        }).collect();
        v.sort_by(|a, b| b.when.cmp(&a.when).then(b.id.cmp(&a.id)));
        v.truncate(q.limit.unwrap_or(200));
        v
    }

    pub fn in_window(&self, from: Day, to: Day) -> impl Iterator<Item = &Decision> {
        self.decisions.values().filter(move |d| d.when >= from && d.when <= to)
    }

    /// Share of decisions in the window whose seven fields are all filled in.
    pub fn completeness(&self, from: Day, to: Day) -> Option<f64> {
        let v: Vec<&Decision> = self.in_window(from, to).collect();
        if v.is_empty() {
            return None;
        }
        let ok = v.iter().filter(|d| is_complete(d)).count();
        Some(ok as f64 / v.len() as f64)
    }
}

pub fn is_complete(d: &Decision) -> bool {
    !d.what.is_empty() && d.when > 0 && !d.who.is_empty() && !d.information.is_empty()
        && !d.assumptions.is_empty() && !d.expected.is_empty()
        && !d.switches.confirm_if.is_empty() && !d.switches.stop_if.is_empty()
}
