//! # ota-core
//!
//! Domain types shared by every crate of the OTA engine: the closed vocabularies of the
//! book (phases, modalities, pathogens, the 13 AI failure modes, Sentinel signal classes,
//! rungs, gate colours...) and the records the engine writes (`Decision`, `ModuleRun`,
//! `DayRecord`, ...). See `docs/DATA.md` for the JSON contract.

pub mod enums;
pub mod records;

pub use enums::*;
pub use records::*;

/// Deterministic id generator: `<prefix>-<day>-<seq>` (e.g. `D-0042-0007`).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct IdGen {
    pub day: Day,
    pub seq: std::collections::BTreeMap<String, u32>,
}

impl IdGen {
    pub fn set_day(&mut self, day: Day) {
        if self.day != day {
            self.day = day;
            self.seq.clear();
        }
    }
    pub fn next(&mut self, prefix: &str) -> String {
        let n = self.seq.entry(prefix.to_string()).or_insert(0);
        *n += 1;
        format!("{prefix}-{:04}-{:04}", self.day, n)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum OtaError {
    #[error("spec error in {path}: {msg}")]
    Spec { path: String, msg: String },
    #[error("expression error: {0}")]
    Expr(String),
    #[error("{0}")]
    Other(String),
}

/// Brier score of a probabilistic forecast.
pub fn brier(p: f64, outcome: bool) -> f64 {
    let o = if outcome { 1.0 } else { 0.0 };
    (p - o).powi(2)
}

/// Day of week for a simulation day given the weekday index (0 = Monday) of day 1.
pub fn weekday_of(day: Day, day1_weekday: u32) -> u32 {
    (day1_weekday + day.saturating_sub(1)) % 7
}

pub const WEEKDAYS: [&str; 7] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn enums_roundtrip() {
        assert_eq!(AiFailureMode::ALL.len(), 13);
        assert_eq!(Pathogen::ALL.len(), 6);
        assert_eq!("Normalisation of deviance".parse::<Pathogen>().unwrap(), Pathogen::NormalisationOfDeviance);
        let k = FindingKind::FailureMode(AiFailureMode::GlassCockpit);
        assert_eq!(serde_json::to_string(&k).unwrap(), r#"{"category":"failure_mode","name":"glass_cockpit"}"#);
        assert!(Rung::ActThenReview.executes_without_approval());
        assert_eq!(Rung::from_level(2), Rung::ActWithApproval);
    }
}
