//! Anomaly journal (observation register, §13.11), double-loop learning, meta-loop and
//! the independent audit.

use crate::calibration::{profile, Calibration};
use crate::ledger::Ledger;
use crate::sentinel::Sentinel;
use ota_core::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

// ---------------------------------------------------------------- anomaly journal

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnomalyJournal {
    pub entries: Vec<Anomaly>,
    /// cluster → day promoted
    pub promoted: BTreeMap<String, Day>,
}

/// Cluster key: explicit cluster, else first tag, else the two longest words.
pub fn cluster_key(a: &Anomaly) -> String {
    if let Some(c) = &a.cluster { return c.clone(); }
    if let Some(t) = a.tags.first() { return t.to_lowercase(); }
    let mut w: Vec<String> = a.text.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| w.len() > 4).map(String::from).collect();
    w.sort_by(|a, b| b.len().cmp(&a.len()).then(a.cmp(b)));
    w.truncate(2);
    w.sort();
    w.join("-")
}

impl AnomalyJournal {
    /// Add entries; promote clusters where ≥3 independent sources agree within 14 days
    /// to an L3 deviation (`metric = "anomaly:<cluster>"`, `detector = "anomaly_cluster"`).
    pub fn add(&mut self, day: Day, items: Vec<Anomaly>, ids: &mut IdGen) -> (Vec<Anomaly>, Vec<Deviation>) {
        let mut added = vec![];
        for mut a in items {
            if a.id.is_empty() { a.id = ids.next("AN"); }
            a.cluster = Some(cluster_key(&a));
            added.push(a.clone());
            self.entries.push(a);
        }
        let mut devs = vec![];
        let clusters: BTreeSet<String> = added.iter().filter_map(|a| a.cluster.clone()).collect();
        for c in clusters {
            if self.promoted.get(&c).map(|d| d + 14 > day).unwrap_or(false) { continue; }
            let recent: Vec<&Anomaly> = self.entries.iter().filter(|a| a.cluster.as_deref() == Some(&c) && a.day + 14 > day).collect();
            let sources: BTreeSet<&str> = recent.iter().map(|a| a.by.as_str()).collect();
            if sources.len() >= 3 {
                self.promoted.insert(c.clone(), day);
                devs.push(Deviation {
                    id: ids.next("DV"), day, metric: format!("anomaly:{c}"), value: recent.len() as f64, baseline: 0.0, sigma: 1.0,
                    z: sources.len() as f64, level: 3, detector: "anomaly_cluster".into(),
                    interpretation: Some(format!("{} independent sources: {}", sources.len(), recent.iter().map(|a| a.text.as_str()).take(3).collect::<Vec<_>>().join(" / "))),
                    routed_to: vec![], suppressed_by_budget: false, disposition: None,
                });
            }
        }
        self.entries.retain(|a| a.day + 180 > day);
        (added, devs)
    }
}

// ---------------------------------------------------------------- double loop

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DoubleLoop {
    /// class → consecutive weeks it was flagged
    pub recurring: BTreeMap<String, u32>,
}

impl DoubleLoop {
    /// Weekly: look at the last 28 days of sealed outcomes. Classes failing repeatedly get
    /// a double-loop finding questioning the *governing* assumptions, not the action.
    pub fn weekly(&mut self, ledger: &Ledger, day: Day, ids: &mut IdGen) -> Vec<Finding> {
        let mut by_class: BTreeMap<&str, (usize, usize, Vec<Id>, BTreeSet<Id>)> = BTreeMap::new();
        for d in ledger.decisions.values() {
            let Some(o) = &d.outcome else { continue };
            if o.sealed_day + 28 < day { continue; }
            let e = by_class.entry(d.class.as_str()).or_default();
            e.0 += 1;
            if o.success == Some(false) {
                e.1 += 1;
                e.2.push(d.id.clone());
            }
            e.3.extend(d.assumptions.iter().cloned());
        }
        let mut out = vec![];
        let mut flagged_now = BTreeSet::new();
        for (class, (n, fails, refs, assumptions)) in by_class {
            if n >= 2 && fails as f64 / n as f64 >= 0.5 {
                flagged_now.insert(class.to_string());
                let weeks = self.recurring.entry(class.to_string()).or_insert(0);
                *weeks += 1;
                let mut s = BTreeMap::new();
                s.insert("sealed".into(), n as f64);
                s.insert("failed".into(), fails as f64);
                s.insert("weeks_recurring".into(), *weeks as f64);
                out.push(Finding {
                    id: ids.next("DL"), day, tool: "double_loop".into(), kind: "recurring_failure".into(),
                    text: format!("{class}: {fails}/{n} outcomes failed in 28 days. Single loop would tune the rule; double loop asks whether the governing assumptions ({}) still hold.",
                                  assumptions.into_iter().collect::<Vec<_>>().join(", ")),
                    refs, stats: s,
                });
            }
        }
        self.recurring.retain(|k, _| flagged_now.contains(k));
        out
    }

    pub fn recurring_count(&self) -> usize {
        self.recurring.values().filter(|w| **w >= 3).count()
    }
}

// ---------------------------------------------------------------- meta-loop

/// Monthly: reads the external research/peer-practice feed and Sentinel's own record and
/// **only proposes** toolkit changes; leadership approves (§13.13).
pub fn meta_loop_context(sentinel: &Sentinel, research: &[EnvEvent], day: Day) -> serde_json::Value {
    let mut by_kind: BTreeMap<String, usize> = BTreeMap::new();
    for f in sentinel.findings.iter().filter(|f| f.day + 30 > day) {
        *by_kind.entry(serde_json::to_value(&f.kind).ok().and_then(|v| v["name"].as_str().map(String::from)).unwrap_or_default()).or_insert(0) += 1;
    }
    serde_json::json!({
        "sentinel_findings_30d": by_kind,
        "sentinel_open": sentinel.findings.iter().filter(|f| f.status == FindingStatus::Open).count(),
        "research": research.iter().map(|e| serde_json::json!({"title": e.title, "text": e.text})).collect::<Vec<_>>(),
    })
}

// ---------------------------------------------------------------- independent audit

/// Monthly audit on a separate, read-only code path: checks Sentinel's and the Lattice's
/// integrity, the instruments, and epistemic corruption of the calibration scores.
pub fn audit(ledger: &Ledger, calibration: &Calibration, sentinel: &Sentinel, lattice: &crate::lattice::Lattice,
             expected_metrics: &[String], published_module_brier: &BTreeMap<String, Option<f64>>, day: Day, ids: &mut IdGen)
    -> (Vec<Finding>, Vec<SentinelFinding>) {
    let mut f = vec![];
    let mut sentinel_out = vec![];
    let mut add = |kind: &str, text: String, refs: Vec<Id>, stats: BTreeMap<String, f64>| {
        f.push(Finding { id: ids.next("AU"), day, tool: "audit".into(), kind: kind.into(), text, refs, stats });
    };
    // 1. Sentinel integrity: findings must reference existing evidence.
    let dangling: Vec<Id> = sentinel.findings.iter().filter(|s| s.day + 31 > day)
        .filter(|s| s.evidence.iter().any(|e| e.starts_with("D-") && ledger.get(e).is_none())).map(|s| s.id.clone()).collect();
    add("sentinel_integrity", if dangling.is_empty() { "Sentinel findings reference existing evidence".into() } else { format!("{} Sentinel finding(s) cite missing evidence", dangling.len()) },
        dangling, BTreeMap::new());
    // 2. Independent recount of accountability (Sentinel's diffusion signal cross-check).
    let unacc: Vec<Id> = ledger.decisions.values().filter(|d| d.when + 31 > day && d.accountable.is_empty()).map(|d| d.id.clone()).collect();
    if !unacc.is_empty() {
        add("accountability", format!("{} decisions in 31 d have no accountable human", unacc.len()), unacc, BTreeMap::new());
    }
    // 3. Instrument audit (lattice L1): expected metrics with no data for 7 days.
    let silent: Vec<String> = expected_metrics.iter().filter(|m| lattice.latest(m).map(|(d, _)| d + 7 < day).unwrap_or(true)).cloned().collect();
    let mut s = BTreeMap::new();
    s.insert("silent_metrics".into(), silent.len() as f64);
    add("instrument_audit", if silent.is_empty() { "all expected metrics reported in the last 7 days".into() } else { format!("no data for 7+ days: {}", silent.join(", ")) },
        silent.clone(), s);
    // 4. Epistemic corruption: recompute module Brier from raw forecasts and compare with
    // the published (tracker) value; a published score better than the raw one is
    // drift in favour of the scored party.
    for (module, published) in published_module_brier {
        let raw = profile(calibration.forecasts.iter().filter(|x| !x.human && x.by == *module)).brier;
        if let (Some(p), Some(r)) = (published, raw) {
            if *p + 0.03 < r {
                let mut st = BTreeMap::new();
                st.insert("published".into(), *p);
                st.insert("recomputed".into(), r);
                sentinel_out.push(SentinelFinding {
                    id: ids.next("SN"), day, kind: FindingKind::FailureMode(AiFailureMode::EpistemicCorruption),
                    signature: format!("{module}: published Brier {p:.3} better than recomputed {r:.3}"), stats: st,
                    evidence: vec![], severity: Severity::High, status: FindingStatus::Open,
                });
            }
        }
    }
    (f, sentinel_out)
}
