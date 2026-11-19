//! # ota-runtime
//!
//! The daily tick (§6) and the [`World`] trait a company implements. One call to
//! [`Engine::run_day`] runs, in order:
//!
//! 1. environment (events + market step) · 2. data-estate ingest · 3. Reality Lattice ·
//! 4. Assumption Cycle + Living OKRs · 5. switches due, outcome sealing, forecasts ·
//! 6. module think · 7. governance · 8. human stand-ins · 9. act ·
//! 10. weekly (calibration, autonomy off-cycle, double loop, coherence, scenarios) ·
//! 11. daily Sentinel + Signal Brief · 12. monthly (autonomy Path 2 review, meta-loop,
//! audit, scenario meeting) · 13. quarterly (autonomy Path 1 review, success grid).
//!
//! Each tool runs only from its `toolkit.md` `enabled_from` day (layered rollout).

mod bus;
mod govern;
mod monitor;
mod state;
mod think;
mod tick;

pub use bus::Bus;
pub use state::{EngineState, HumanInput};

use anyhow::{Context as _, Result};
use ota_core::*;
use ota_llm::{LlmClient, LlmMode, LlmRequest, LlmResult, Scripted};
use ota_spec::CompanySpec;
use ota_store::Store;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

/// What the environment produced for one day.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorldDay {
    /// Telemetry and KPIs (metric → value). Become internal `Signal`s.
    pub kpis: BTreeMap<String, f64>,
    /// Additional signals (reviews, store notices, ...). Provenance is kept as given.
    pub signals: Vec<Signal>,
    /// Observation-register entries written by end nodes or modules (§13.11).
    pub anomalies: Vec<Anomaly>,
    /// External "research and peer practice" feed for the meta-loop (§13.13).
    pub research: Vec<EnvEvent>,
    /// Human actions taken outside the ledger today (Sentinel: bypass).
    pub untraced_human_actions: usize,
    pub notes: Vec<String>,
}

/// Access to the LLM for environment event generation (tier `env` when live; the
/// world's scripted responder otherwise).
pub struct EventCtx<'a> {
    pub day: Day,
    pub date: String,
    pub spec: &'a CompanySpec,
    pub recent_events: &'a [EnvEvent],
    pub recent_kpis: &'a BTreeMap<String, f64>,
    llm: &'a mut LlmClient,
}

impl EventCtx<'_> {
    pub fn llm(&mut self, req: &LlmRequest) -> LlmResult {
        self.llm.call(req)
    }
    pub fn is_live(&self) -> bool {
        self.llm.is_live()
    }
}

/// Implemented by the company (market model, actions, scripted responses).
pub trait World: Send {
    /// Called once after the engine is created or restored.
    fn init(&mut self, _spec: &CompanySpec) -> Result<()> {
        Ok(())
    }
    /// Step 1a: propose today's external events (0–6). Use `ctx.llm` with tier `env`
    /// for live generation; return scripted/storyline events otherwise.
    fn generate_events(&mut self, day: Day, ctx: &mut EventCtx) -> Vec<EnvEvent>;
    /// Step 1b: advance the environment one day, given today's events and the actions
    /// executed yesterday. Returns signals, telemetry and KPIs.
    fn advance(&mut self, day: Day, events: &[EnvEvent], executed: &[Action]) -> Result<WorldDay>;
    /// Step 9: apply an approved action to the company state. Returns a short effect
    /// summary. Effects reach the environment the next day.
    fn apply_action(&mut self, day: Day, action: &Action) -> Result<String>;
    /// Deterministic responder for every LLM purpose (history, offline, budget fallback).
    fn scripted(&self) -> Arc<dyn Scripted>;
    /// Serialise / restore the world so live runs can resume.
    fn save(&self) -> Value;
    fn load(&mut self, state: Value) -> Result<()>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineOptions {
    pub data_dir: PathBuf,
    pub llm_mode: LlmMode,
    /// Days up to and including this one always use the scripted responder (history).
    pub scripted_until: Day,
    /// Persist full tool state every N days (and always at the end of a run).
    pub checkpoint_every: Day,
    /// Directory for the LLM response cache (default `<data_dir>/llm-cache`).
    pub cache_dir: Option<PathBuf>,
}

impl EngineOptions {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        EngineOptions { data_dir: data_dir.into(), llm_mode: LlmMode::Auto, scripted_until: 0, checkpoint_every: 30, cache_dir: None }
    }
}

/// Progress callback: (day just completed, days done in this run, days requested).
pub type Progress<'a> = &'a mut dyn FnMut(&DayRecord, u32, u32);

pub struct Engine {
    pub spec: CompanySpec,
    pub store: Store,
    pub llm: LlmClient,
    pub world: Box<dyn World>,
    pub st: EngineState,
    pub opts: EngineOptions,
}

impl Engine {
    /// Create an engine over `data_dir`, resuming from saved state if present.
    pub fn new(spec: CompanySpec, mut world: Box<dyn World>, opts: EngineOptions) -> Result<Engine> {
        let store = Store::open(&opts.data_dir)?;
        let cache = opts.cache_dir.clone().unwrap_or_else(|| opts.data_dir.join("llm-cache"));
        let llm = LlmClient::new(spec.llm.clone(), world.scripted(), opts.llm_mode, Some(cache));
        let st = match state::load(&store)? {
            Some(st) => {
                if let Some(w) = store.load_state::<Value>("world")? {
                    world.load(w).context("restoring world state")?;
                }
                st
            }
            None => EngineState::fresh(&spec),
        };
        world.init(&spec)?;
        let mut e = Engine { spec, store, llm, world, st, opts };
        e.st.sync_spec(&e.spec);
        e.write_meta()?;
        Ok(e)
    }

    /// Last completed day (0 before the first tick).
    pub fn day(&self) -> Day {
        self.st.day
    }

    pub fn date_of(&self, day: Day) -> chrono::NaiveDate {
        let (y, m, d) = ota_spec::loader::chrono_like_date(&self.spec.company.start_date).unwrap_or((2025, 1, 1));
        chrono::NaiveDate::from_ymd_opt(y, m, d).unwrap_or_default() + chrono::Duration::days(day as i64 - 1)
    }

    /// Run `n` days. Persists every day record; checkpoints state periodically and at the end.
    pub fn run(&mut self, n: u32, progress: Option<Progress>) -> Result<Vec<Day>> {
        let mut done = vec![];
        let mut progress = progress;
        for i in 0..n {
            let rec = self.run_day()?;
            done.push(rec.day);
            if let Some(p) = progress.as_mut() {
                p(&rec, i + 1, n);
            }
            if self.opts.checkpoint_every > 0 && rec.day % self.opts.checkpoint_every == 0 {
                self.checkpoint()?;
            }
        }
        self.checkpoint()?;
        Ok(done)
    }

    /// Save all tool state, the world, and the public `/api/state` view.
    pub fn checkpoint(&mut self) -> Result<()> {
        state::save(&self.store, &self.st)?;
        self.store.save_state("world", &self.world.save())?;
        self.store.save_state("view", &monitor::state_view(self))?;
        for p in self.spec.people.iter().filter(|p| p.role != NodeKind::AiModule) {
            self.store.save_private(&p.id, &serde_json::json!({
                "person": p.id, "visibility": "private: only this person may read it (§13.5)",
                "calibration": self.st.calibration.person(&p.id),
            }))?;
        }
        Ok(())
    }

    /// `data/meta.json`: everything static the viewer needs.
    pub fn write_meta(&self) -> Result<()> {
        let s = &self.spec;
        let meta = serde_json::json!({
            "engine_version": env!("CARGO_PKG_VERSION"),
            "company": s.company,
            "constitution": s.constitution,
            "toolkit": s.toolkit,
            "modules": s.modules.iter().map(|m| serde_json::json!({
                "id": m.id, "name": m.name, "org": m.org, "kind": m.kind, "phases": m.phases, "cadence": m.cadence,
                "inputs": m.inputs, "outputs": m.outputs, "owner": m.owner, "accountable": m.accountable,
                "evolves_from": m.evolves_from, "depends_on": m.depends_on, "alert_budget": m.alert_budget,
                "modality": m.modality, "tool": m.tool, "metric_owner_of": m.metric_owner_of, "classes": m.classes,
                "description": m.body, "prompts": m.prompts.keys().collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
            "people": s.people.iter().map(|p| serde_json::json!({
                "id": p.id, "name": p.name, "role": p.role, "title": p.title, "accountable_for": p.accountable_for,
                "expertise": p.expertise, "llm_tier": p.llm_tier, "weekly_capacity_min": p.weekly_capacity_min,
                "bias": p.bias, "shadow_group": p.shadow_group, "persona": p.persona, "judgment": p.judgment,
                "gate_rules": p.gate_rules,
            })).collect::<Vec<_>>(),
            "classes": s.classes.values().map(|c| serde_json::json!({
                "id": c.id, "module": c.module, "accountable": c.accountable, "reversibility": c.reversibility,
                "impact": c.impact, "path": c.path, "default_rung": c.default_rung, "stakes_eur": c.stakes_eur,
                "novelty": c.novelty, "kind": c.kind, "modality": c.modality, "human_only": c.human_only,
                "validator": c.validator, "assumptions": c.assumptions, "forecast": c.forecast,
                "confirm_if": c.confirm_if, "stop_if": c.stop_if, "check_after_days": c.check_after_days,
                "rows": c.rows, "description": c.description,
            })).collect::<Vec<_>>(),
            "kpis": s.kpis,
            "llm": s.llm.public_view(),
            "llm_mode": self.llm.mode_label(),
            "scripted_until": self.opts.scripted_until,
            "warnings": s.warnings,
        });
        self.store.write_json("meta.json", &meta)
    }
}

/// Topological order of modules: `depends_on` first, observe-phase modules before
/// others, then by id.
pub fn module_order(spec: &CompanySpec) -> Vec<String> {
    let mut ids: Vec<&ota_spec::ModuleSpec> = spec.modules.iter().collect();
    ids.sort_by_key(|m| (!m.phases.contains(&Phase::Observe), m.id.clone()));
    let mut out: Vec<String> = vec![];
    let mut visiting: Vec<String> = vec![];
    fn visit(m: &ota_spec::ModuleSpec, spec: &CompanySpec, out: &mut Vec<String>, visiting: &mut Vec<String>) {
        if out.contains(&m.id) || visiting.contains(&m.id) {
            return;
        }
        visiting.push(m.id.clone());
        for d in &m.depends_on {
            if let Some(dm) = spec.module(d) {
                visit(dm, spec, out, visiting);
            }
        }
        out.push(m.id.clone());
    }
    for m in ids {
        visit(m, spec, &mut out, &mut visiting);
    }
    out
}
