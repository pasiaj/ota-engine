//! # ota-tools — the book's toolkit as stateful services
//!
//! | Tool (book) | Module |
//! |---|---|
//! | Decision Ledger (7 fields, switches, outcome sealing, search) | [`ledger`] |
//! | Assumption Cycle (+ graveyard) | [`assumptions`] |
//! | Adversarial Engine, Auto Pre-Mortem, Domain Expert Gate, stand-in schema | [`governance`] |
//! | Living Scenario Tree (DAG, major/minor/urgent) | [`scenarios`] |
//! | Strategy Coherence (incl. regulation claims) | [`coherence`] |
//! | Living OKRs (flag to setter) | [`okrs`] |
//! | Calibration Tracker (Brier; private human profiles) | [`calibration`] |
//! | Autonomy Ladder | [`autonomy`] |
//! | Reality Lattice L1–L6 + alert budget | [`lattice`] |
//! | Sentinel | [`sentinel`] |
//! | Anomaly journal, Double-loop, Meta-loop, Independent audit | [`learning`] |
//! | Success grid | [`grid`] |
//! | Per-day aggregates for monitors | [`history`] |
//!
//! The Signal Brief and the daily metrics are assembled by `ota-runtime` from these.

pub mod assumptions;
pub mod autonomy;
pub mod calibration;
pub mod coherence;
pub mod governance;
pub mod grid;
pub mod history;
pub mod lattice;
pub mod learning;
pub mod ledger;
pub mod okrs;
pub mod scenarios;
pub mod sentinel;
