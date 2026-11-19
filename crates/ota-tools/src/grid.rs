//! Success-grid self-assessment: 3 phases × 5 modalities, compared with the OTA 200
//! profiles (ota.js `figures.grid`), with zones (universal anchor = think × direction,
//! culture trap = culture column), archetype classification and residual risk.

use crate::history::History;
use ota_core::*;
use std::collections::BTreeMap;

/// OTA 200 weight profiles (% of total weight per cell), rows observe/think/act,
/// cols direction/structure/processes/capability/culture.
pub const PROFILES: &[(&str, [[f64; 5]; 3])] = &[
    ("failures_all", [[2.0, 3.5, 3.1, 0.6, 5.0], [16.7, 14.3, 8.4, 1.3, 21.5], [4.6, 5.4, 5.4, 0.6, 7.8]]),
    ("failures_nonfraud", [[2.5, 2.5, 3.1, 0.8, 3.6], [20.0, 13.1, 10.0, 1.9, 18.6], [5.5, 4.7, 6.7, 0.8, 6.2]]),
    ("failures_fraud", [[0.8, 5.8, 3.0, 0.0, 8.3], [8.8, 16.9, 4.7, 0.0, 28.4], [2.5, 7.0, 2.4, 0.0, 11.4]]),
    ("successes", [[10.9, 2.3, 4.0, 6.2, 4.6], [18.2, 4.1, 6.2, 8.5, 8.8], [9.9, 2.4, 4.5, 5.0, 4.6]]),
];

/// Which grid cells each tool covers (phase row, modality col).
pub const TOOL_CELLS: &[(&str, &[(usize, usize)])] = &[
    ("ledger", &[(2, 2), (1, 2)]),
    ("assumptions", &[(1, 0), (0, 0)]),
    ("adversarial", &[(1, 0), (1, 4)]),
    ("premortem", &[(1, 0), (1, 1)]),
    ("lattice", &[(0, 2), (0, 0), (0, 3)]),
    ("anomalies", &[(0, 4)]),
    ("brief", &[(0, 1)]),
    ("scenarios", &[(1, 0), (0, 0)]),
    ("calibration", &[(1, 3)]),
    ("autonomy", &[(2, 1), (2, 3)]),
    ("gate", &[(1, 3), (2, 3)]),
    ("okrs", &[(2, 0)]),
    ("coherence", &[(1, 1), (2, 1), (2, 0)]),
    ("double_loop", &[(1, 2), (2, 4)]),
    ("sentinel", &[(0, 4), (1, 4)]),
    ("meta_loop", &[(1, 2)]),
    ("audit", &[(0, 2), (0, 1)]),
];

pub fn assess(day: Day, quarter: String, enabled: &[String], history: &History, ledger_completeness: Option<f64>) -> GridScore {
    let mut cover = [[0usize; 5]; 3];
    for (t, cells) in TOOL_CELLS {
        if enabled.iter().any(|e| e == t) {
            for (r, c) in *cells {
                cover[*r][*c] += 1;
            }
        }
    }
    let from = day.saturating_sub(90);
    let ext = history.mean(from, day, |a| a.external_share).unwrap_or(0.3);
    let decisions = history.sum(from, day, |a| a.decisions).max(1) as f64;
    let dissent = (history.sum(from, day, |a| a.challenges + a.human_rejections) as f64 / decisions).min(1.0);
    let real_human = history.sum(from, day, |a| a.real_human_actions) as f64 / history.sum(from, day, |a| a.human_actions).max(1) as f64;
    let mut cells = [[0.0; 5]; 3];
    for r in 0..3 {
        for c in 0..5 {
            cells[r][c] = 0.25 + 0.25 * (cover[r][c].min(3) as f64);
        }
    }
    // behaviour adjustments
    cells[0][0] = (cells[0][0] * (0.6 + ext)).min(1.0); // observe × direction needs outside view
    cells[2][2] *= 0.5 + 0.5 * ledger_completeness.unwrap_or(0.5);
    for r in 0..3 {
        cells[r][4] *= 0.6 + 0.4 * (dissent * 4.0).min(1.0); // culture: measured as dissent [deviation]
    }
    cells[1][4] *= 0.8 + 0.2 * real_human.min(1.0);
    for row in cells.iter_mut() {
        for v in row.iter_mut() {
            *v = v.clamp(0.0, 1.0);
        }
    }
    // residual risk: failure weights × weakness, normalised to 100
    let fail = PROFILES[1].1;
    let mut residual = [[0.0; 5]; 3];
    let mut total = 0.0;
    for r in 0..3 {
        for c in 0..5 {
            residual[r][c] = fail[r][c] * (1.0 - cells[r][c]);
            total += residual[r][c];
        }
    }
    if total > 0.0 {
        for row in residual.iter_mut() {
            for v in row.iter_mut() {
                *v = *v / total * 100.0;
            }
        }
    }
    // weakness profile vs OTA 200 failure/success weight profiles
    let mut weak = [[0.0; 5]; 3];
    let wsum: f64 = cells.iter().flatten().map(|v| 1.0 - v).sum::<f64>().max(1e-9);
    for r in 0..3 {
        for c in 0..5 {
            weak[r][c] = (1.0 - cells[r][c]) / wsum * 100.0;
        }
    }
    let mut dist = BTreeMap::new();
    for (name, p) in PROFILES {
        let d: f64 = (0..3).flat_map(|r| (0..5).map(move |c| (r, c))).map(|(r, c)| (weak[r][c] - p[r][c]).abs()).sum();
        dist.insert(name.to_string(), d);
    }
    let nearest = dist.iter().min_by(|a, b| a.1.partial_cmp(b.1).unwrap()).map(|x| x.0.clone()).unwrap_or_default();
    let col = |c: usize| (0..3).map(|r| cells[r][c]).sum::<f64>() / 3.0;
    let archetype = if col(4) < 0.45 {
        "Culture orphan (Kulttuuriorpo)"
    } else if col(0) + 0.1 < (col(2) + col(3)) / 2.0 {
        "Execution empire (Toteutusimperiumi)"
    } else if col(1) + 0.1 < col(0) {
        "Growth-company founder (Kasvuyrityksen perustaja)"
    } else {
        "balanced"
    };
    let mut notes = vec![];
    if cells[1][0] < 0.6 { notes.push("universal anchor (think × direction) is weak".to_string()); }
    if col(4) < 0.5 { notes.push("culture trap: culture column weak; tools cannot generate culture (ch 16)".to_string()); }
    GridScore {
        quarter, day, cells, residual_risk: residual, universal_anchor: cells[1][0], culture_trap: col(4),
        nearest_profile: nearest, profile_distance: dist, archetype: archetype.into(), notes,
    }
}
