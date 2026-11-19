//! Strategy Coherence Engine: claims (mission → strategy → OKR → budget → incentive →
//! policy, plus regulation, §13.14) checked against what the ledger actually does.

use crate::ledger::Ledger;
use ota_core::*;
use ota_spec::CompanySpec;
use std::collections::BTreeMap;

pub struct CoherenceReport {
    pub findings: Vec<Finding>,
    /// Largest |actual − stated| budget share gap (misleading-incentives signal).
    pub budget_gap: f64,
}

pub fn check(spec: &CompanySpec, ledger: &Ledger, okrs: &[OkrState], day: Day, ids: &mut IdGen) -> CoherenceReport {
    let mut findings = vec![];
    let from = day.saturating_sub(28);
    let window: Vec<&Decision> = ledger.in_window(from, day).collect();
    let mut f = |kind: &str, text: String, refs: Vec<Id>, stats: BTreeMap<String, f64>| {
        findings.push(Finding { id: ids.next("CO"), day, tool: "coherence".into(), kind: kind.into(), text, refs, stats });
    };

    // dangling links
    for c in &spec.claims {
        for l in &c.links {
            if !spec.claims.iter().any(|x| &x.id == l) {
                f("dangling_link", format!("claim {} links to unknown claim {l}", c.id), vec![c.id.clone()], BTreeMap::new());
            }
        }
    }

    // budget: stated spend share by org vs ledger stakes
    let org_of = |m: &str| spec.module(m).map(|x| x.org.clone()).unwrap_or_default();
    let total: f64 = window.iter().map(|d| d.stakes_eur).sum();
    let mut budget_gap: f64 = 0.0;
    if total > 0.0 {
        for c in spec.claims.iter().filter(|c| c.kind == ClaimKind::Budget) {
            let (Some(org), Some(share)) = (c.attrs.get("org").and_then(|v| v.as_str()), c.attrs.get("spend_share").and_then(|v| v.as_f64())) else { continue };
            let actual: f64 = window.iter().filter(|d| org_of(&d.module) == org).map(|d| d.stakes_eur).sum::<f64>() / total;
            let gap = (actual - share).abs();
            budget_gap = budget_gap.max(gap);
            if gap > 0.2 {
                let mut s = BTreeMap::new();
                s.insert("stated".into(), share);
                s.insert("actual".into(), actual);
                f("budget_gap", format!("{org}: stated spend share {:.0}% but ledger shows {:.0}% (28 d)", share * 100.0, actual * 100.0), vec![c.id.clone()], s);
            }
        }
    }

    // regulation: decisions of covered classes must satisfy the requirement
    for c in spec.claims.iter().filter(|c| c.kind == ClaimKind::Regulation) {
        let applies: Vec<String> = c.attrs.get("applies_to").and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
        let req = c.attrs.get("requires").and_then(|v| v.as_str()).unwrap_or("gate");
        let covers = |cls: &str| applies.iter().any(|a| a == cls || (a.ends_with(".*") && cls.starts_with(&a[..a.len() - 1])));
        let bad: Vec<Id> = window.iter().filter(|d| covers(&d.class)).filter(|d| match req {
            "gate" => d.gate.as_ref().map(|g| g.color == GateColor::Green && g.rule == "no rule matched").unwrap_or(true),
            "premortem" => d.premortem.is_none(),
            "human_approval" => !d.approvals.iter().any(|a| a.verdict == HumanVerdict::Approve),
            _ => false,
        }).map(|d| d.id.clone()).collect();
        if !bad.is_empty() {
            let mut s = BTreeMap::new();
            s.insert("violations".into(), bad.len() as f64);
            f("regulation_gap", format!("{}: {} decision(s) lack required `{req}` ({})", c.id, bad.len(), c.text), bad, s);
        }
    }

    // OKRs with no supporting decision in 28 days
    for o in okrs {
        let metrics: Vec<&String> = o.key_results.iter().map(|k| &k.metric).collect();
        let supported = window.iter().any(|d| spec.module(&d.module).map(|m| m.inputs.iter().any(|i| metrics.contains(&i))).unwrap_or(false));
        if !supported && day > 28 {
            f("okr_unsupported", format!("OKR {} ({}) has no supporting decisions in 28 days", o.id, o.objective), vec![o.id.clone()], BTreeMap::new());
        }
    }
    CoherenceReport { findings, budget_gap }
}
