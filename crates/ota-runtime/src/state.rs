//! Engine state: all tool state plus runtime bookkeeping. Saved as one file per tool
//! under `data/state/` (internal; the public view is `state/view.json`).

use crate::bus::Bus;
use anyhow::Result;
use ota_core::*;
use ota_spec::CompanySpec;
use ota_store::Store;
use ota_tools::{assumptions::AssumptionCycle, autonomy::Autonomy, calibration::Calibration, history::History,
                lattice::Lattice, learning::{AnomalyJournal, DoubleLoop}, ledger::Ledger, scenarios::ScenarioTree,
                sentinel::Sentinel};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Live input from a real human through the viewer (§13.17). Resolves a queue item or a
/// decision, or injects a framing message from a leader.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanInput {
    pub person: NodeId,
    #[serde(default)]
    pub item: Option<Id>,
    #[serde(default)]
    pub decision: Option<Id>,
    #[serde(default)]
    pub verdict: Option<HumanVerdict>,
    #[serde(default)]
    pub reason: String,
    /// Free-text framing (goal, constraint, question) sent to modules/leaders.
    #[serde(default)]
    pub framing: Option<String>,
    #[serde(default)]
    pub to: Vec<NodeId>,
    #[serde(default)]
    pub submitted_at: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Runtime {
    pub day: Day,
    pub ids: IdGen,
    pub queue: Vec<QueueItem>,
    /// node → deviation ids that went over the alert budget and need "dismiss or act".
    pub alert_queue: BTreeMap<NodeId, Vec<Id>>,
    /// Deviations still awaiting disposition (by id), so their records can be updated.
    pub open_devs: BTreeMap<Id, Deviation>,
    /// name → last day seen (decision class, action kind, event kind/tag). For days_since().
    pub last_seen: BTreeMap<String, Day>,
    /// Actions executed yesterday, fed to `World::advance` today.
    pub pending_actions: Vec<Action>,
    pub recent_events: Vec<EnvEvent>,
    pub recent_kpis: BTreeMap<String, f64>,
    pub bus: Bus,
    pub okrs: Vec<OkrState>,
    pub findings: Vec<Finding>,
    pub coherence_gap: Option<f64>,
    pub grid: Vec<GridScore>,
    pub briefs: Vec<SignalBrief>,
    /// Module Brier as published by the calibration tracker (checked by the audit).
    pub published_module_brier: BTreeMap<String, Option<f64>>,
    /// Scenario proposals by metric → day (dedupe).
    pub scenario_proposed: BTreeMap<String, Day>,
    pub research: Vec<EnvEvent>,
    /// Adversarial challenges of the last 90 days (approvals may come days later).
    pub challenges: BTreeMap<Id, Challenge>,
}

#[derive(Debug, Clone, Default)]
pub struct EngineState {
    pub day: Day,
    pub rt: Runtime,
    pub ledger: Ledger,
    pub assumptions: AssumptionCycle,
    pub scenarios: ScenarioTree,
    pub calibration: Calibration,
    pub autonomy: Autonomy,
    pub lattice: Lattice,
    pub sentinel: Sentinel,
    pub anomalies: AnomalyJournal,
    pub double_loop: DoubleLoop,
    pub history: History,
}

impl EngineState {
    pub fn fresh(spec: &CompanySpec) -> Self {
        let mut st = EngineState {
            assumptions: AssumptionCycle::from_specs(&spec.assumptions, 1),
            scenarios: ScenarioTree { nodes: spec.scenarios.clone() },
            ..Default::default()
        };
        st.rt.okrs = spec.okrs.clone();
        st.autonomy.init(&spec.classes);
        st
    }

    /// Pick up spec additions after a restart (new classes, assumptions).
    pub fn sync_spec(&mut self, spec: &CompanySpec) {
        self.autonomy.init(&spec.classes);
        self.assumptions.sync_specs(&spec.assumptions, self.day.max(1));
        for o in &spec.okrs {
            if !self.rt.okrs.iter().any(|x| x.id == o.id) {
                self.rt.okrs.push(o.clone());
            }
        }
        let wd = crate::Engine::weekday_index(spec);
        self.lattice.day1_weekday = wd;
    }
}

pub fn save(store: &Store, st: &EngineState) -> Result<()> {
    let mut rt = st.rt.clone();
    rt.day = st.day;
    store.save_state("engine", &rt)?;
    store.save_state("ledger", &st.ledger)?;
    store.save_state("assumptions", &st.assumptions)?;
    store.save_state("scenarios", &st.scenarios)?;
    store.save_state("calibration", &st.calibration)?;
    store.save_state("autonomy", &st.autonomy)?;
    store.save_state("lattice", &st.lattice)?;
    store.save_state("sentinel", &st.sentinel)?;
    store.save_state("anomalies", &st.anomalies)?;
    store.save_state("double_loop", &st.double_loop)?;
    store.save_state("history", &st.history)?;
    Ok(())
}

pub fn load(store: &Store) -> Result<Option<EngineState>> {
    let Some(rt) = store.load_state::<Runtime>("engine")? else { return Ok(None) };
    Ok(Some(EngineState {
        day: rt.day,
        ledger: store.load_state("ledger")?.unwrap_or_default(),
        assumptions: store.load_state("assumptions")?.unwrap_or_default(),
        scenarios: store.load_state("scenarios")?.unwrap_or_default(),
        calibration: store.load_state("calibration")?.unwrap_or_default(),
        autonomy: store.load_state("autonomy")?.unwrap_or_default(),
        lattice: store.load_state("lattice")?.unwrap_or_default(),
        sentinel: store.load_state("sentinel")?.unwrap_or_default(),
        anomalies: store.load_state("anomalies")?.unwrap_or_default(),
        double_loop: store.load_state("double_loop")?.unwrap_or_default(),
        history: store.load_state("history")?.unwrap_or_default(),
        rt,
    }))
}

/// Read and remove pending real-human inputs from `data/inbox/*.json`.
pub fn take_human_inputs(store: &Store) -> Vec<HumanInput> {
    let dir = store.root.join("inbox");
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&dir).map(|rd| rd.flatten().map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("json")).collect()).unwrap_or_default();
    files.sort();
    let mut out = vec![];
    for f in files {
        if let Ok(s) = std::fs::read_to_string(&f) {
            if let Ok(h) = serde_json::from_str::<HumanInput>(&s) {
                out.push(h);
            }
        }
        let _ = std::fs::remove_file(&f);
    }
    out
}
