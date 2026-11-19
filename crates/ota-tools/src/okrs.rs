//! Living OKRs: key results track KPIs daily; when an underlying assumption is refuted
//! or a key result drifts off course, the OKR is flagged **to its setter** (ch 13).

use ota_core::*;
use std::collections::BTreeMap;

/// Update KR currents and return OKRs whose flag changed today.
pub fn update(okrs: &mut [OkrState], kpis: &BTreeMap<String, f64>, cycle: &crate::assumptions::AssumptionCycle, day: Day) -> Vec<OkrState> {
    let mut flagged = vec![];
    for o in okrs.iter_mut() {
        let mut reasons = vec![];
        for kr in o.key_results.iter_mut() {
            if let Some(v) = kpis.get(&kr.metric) {
                kr.current = Some(*v);
                let off = if kr.direction == "down" { *v > kr.target * 1.3 } else { *v < kr.target * 0.7 };
                if off {
                    reasons.push(format!("{} at {:.3} vs target {:.3}", kr.metric, v, kr.target));
                }
            }
        }
        for a in &o.assumptions {
            if let Some(x) = cycle.get(a) {
                match x.status {
                    AssumptionStatus::Refuted => reasons.push(format!("assumption {a} refuted")),
                    AssumptionStatus::Stale => reasons.push(format!("assumption {a} stale")),
                    _ => {}
                }
            }
        }
        let new = (!reasons.is_empty()).then(|| reasons.join("; "));
        if new != o.flagged {
            let raise = new.is_some();
            o.flagged = new;
            o.flagged_day = raise.then_some(day);
            if raise {
                flagged.push(o.clone());
            }
        }
    }
    flagged
}
