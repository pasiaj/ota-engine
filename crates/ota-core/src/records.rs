//! Records written by the engine. All are plain serde structs; the JSON shapes are the
//! contract documented in `docs/DATA.md`.

use crate::enums::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub type Day = u32;
/// Minute within the simulated day (0..1440).
pub type Ts = u32;
pub type Id = String;
pub type NodeId = String;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Signal {
    pub id: Id,
    pub day: Day,
    pub source: String,
    pub kind: String,
    #[serde(default)]
    pub metric: Option<String>,
    #[serde(default)]
    pub value: Option<f64>,
    #[serde(default)]
    pub text: String,
    pub provenance: Provenance,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl Default for Provenance {
    fn default() -> Self {
        Provenance::Internal
    }
}

/// An external environment event (weak signal source). Produced by the World.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct EnvEvent {
    pub id: Id,
    pub day: Day,
    pub kind: String,
    pub title: String,
    #[serde(default)]
    pub text: String,
    /// Free-form parameter changes the world applied (e.g. {"cpi_mult": 1.2, "half_life_days": 14}).
    #[serde(default)]
    pub effects: Value,
    #[serde(default)]
    pub tags: Vec<String>,
    /// "llm" | "scripted" | "storyline" ...
    #[serde(default)]
    pub generated_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Deviation {
    pub id: Id,
    pub day: Day,
    pub metric: String,
    pub value: f64,
    pub baseline: f64,
    pub sigma: f64,
    pub z: f64,
    /// Highest Reality Lattice level reached (1..=6, §13.8).
    pub level: u8,
    /// "z" | "trend_break" | "coverage_gap" | "anomaly_cluster"
    pub detector: String,
    #[serde(default)]
    pub interpretation: Option<String>,
    #[serde(default)]
    pub routed_to: Vec<NodeId>,
    #[serde(default)]
    pub suppressed_by_budget: bool,
    /// Set when the recipient explicitly disposes of the alert (§13.15).
    #[serde(default)]
    pub disposition: Option<AlertDisposition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlertDisposition {
    pub day: Day,
    pub by: NodeId,
    /// "acted" | "dismissed"
    pub action: String,
    pub reason: String,
    #[serde(default)]
    pub decision: Option<Id>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Message {
    pub id: Id,
    pub day: Day,
    pub ts: Ts,
    pub from: NodeId,
    pub to: Vec<NodeId>,
    pub intent: Intent,
    pub subject: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub refs: Vec<Id>,
    /// Number of nodes this information passed through before reaching `to`.
    pub hops: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Forecast {
    pub id: Id,
    pub day: Day,
    pub by: NodeId,
    pub question: String,
    pub p: f64,
    pub resolves_on: Day,
    #[serde(default)]
    pub outcome: Option<bool>,
    #[serde(default)]
    pub brier: Option<f64>,
    pub topic: String,
    pub horizon_days: u32,
    #[serde(default)]
    pub decision: Option<Id>,
    /// Expression that resolves the forecast (true = event happened).
    #[serde(default)]
    pub resolve_if: Option<String>,
    /// True when `by` is a human; such forecasts feed private calibration profiles.
    #[serde(default)]
    pub human: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndicatorRule {
    pub expr: String,
    /// "confirm" | "refute"
    pub effect: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfidencePoint {
    pub day: Day,
    pub confidence: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Assumption {
    pub id: Id,
    pub text: String,
    pub owner: NodeId,
    #[serde(default)]
    pub module: Option<String>,
    pub confidence: f64,
    pub load_bearing: bool,
    pub created: Day,
    pub last_confirmed: Day,
    pub status: AssumptionStatus,
    #[serde(default)]
    pub indicators: Vec<IndicatorRule>,
    #[serde(default)]
    pub depends: Vec<Id>,
    #[serde(default)]
    pub history: Vec<ConfidencePoint>,
    /// Day the assumption went stale (graveyard tracking).
    #[serde(default)]
    pub stale_since: Option<Day>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Challenge {
    pub id: Id,
    pub day: Day,
    /// Assumption or decision id.
    pub target: Id,
    pub decision: Id,
    pub argument: String,
    #[serde(default)]
    pub sources: Vec<String>,
    pub strength: f64,
    #[serde(default)]
    pub response: Option<String>,
    #[serde(default)]
    pub overridden_by: Option<NodeId>,
    #[serde(default)]
    pub accepted: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FailureModeItem {
    pub text: String,
    pub p: f64,
    pub severity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PreMortem {
    pub id: Id,
    pub decision: Id,
    pub day: Day,
    pub started_ts: Ts,
    #[serde(default)]
    pub failure_modes: Vec<FailureModeItem>,
    #[serde(default)]
    pub load_bearing: Vec<Id>,
    pub skipped: bool,
    #[serde(default)]
    pub skip_reason: Option<String>,
    pub tier: Tier,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DecisionSwitch {
    pub confirm_if: String,
    pub stop_if: String,
    pub check_by: Day,
}

/// The four elements of action (ch 4), required on test/launch/scale decisions (§13.9).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ActionElements {
    #[serde(default)]
    pub hypothesis: Option<String>,
    #[serde(default)]
    pub learning_metric: Option<String>,
    #[serde(default)]
    pub decision_point: Option<String>,
    #[serde(default)]
    pub reversibility: Option<String>,
}

impl ActionElements {
    pub fn missing(&self) -> Vec<&'static str> {
        let mut m = vec![];
        let empty = |o: &Option<String>| o.as_deref().map(str::trim).unwrap_or("").is_empty();
        if empty(&self.hypothesis) { m.push("hypothesis"); }
        if empty(&self.learning_metric) { m.push("learning_metric"); }
        if empty(&self.decision_point) { m.push("decision_point"); }
        if empty(&self.reversibility) { m.push("reversibility"); }
        m
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GateReview {
    pub color: GateColor,
    pub rule: String,
    pub validator: Option<NodeId>,
    /// Who performed the review for red: a validator id, or "ai" under Path 2 authority.
    #[serde(default)]
    pub reviewed_by: Option<NodeId>,
    #[serde(default)]
    pub reviewed_day: Option<Day>,
    #[serde(default)]
    pub verdict: Option<HumanVerdict>,
    #[serde(default)]
    pub recommendation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Approval {
    pub by: NodeId,
    pub day: Day,
    pub ts: Ts,
    pub verdict: HumanVerdict,
    pub reason: String,
    pub minutes_spent: f64,
    pub source: HumanSource,
    #[serde(default)]
    pub amendments: Option<String>,
    /// For a post-hoc review of an act_then_review decision.
    #[serde(default)]
    pub post_hoc: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Override {
    pub by: NodeId,
    pub day: Day,
    /// What was overridden: "challenge:<id>", "recommendation", "gate"...
    pub what: String,
    pub reason: String,
    pub source: HumanSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Outcome {
    pub sealed_day: Day,
    pub actual: String,
    pub expected: String,
    pub deviation: String,
    pub success: Option<bool>,
    pub confirm_hit: bool,
    pub stop_hit: bool,
    #[serde(default)]
    pub metrics: BTreeMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuleHit {
    pub class: String,
    pub row: usize,
    pub when: String,
    pub then: String,
    pub confidence: f64,
    #[serde(default)]
    pub escalate: Option<Tier>,
    /// Values of identifiers referenced in `when` at evaluation time.
    #[serde(default)]
    pub bindings: BTreeMap<String, f64>,
}

/// Parsed action from a decision-table `then` cell or an LLM decision.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActionSpec {
    /// "hold" means no action. Otherwise a company-defined action kind.
    pub kind: String,
    #[serde(default)]
    pub params: Value,
}

impl ActionSpec {
    pub fn hold() -> Self {
        ActionSpec { kind: "hold".into(), params: Value::Null }
    }
    pub fn is_hold(&self) -> bool {
        matches!(self.kind.as_str(), "hold" | "none" | "noop" | "")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Decision {
    pub id: Id,
    pub class: String,
    pub module: String,
    // --- the seven fields ---
    pub what: String,
    pub when: Day,
    pub who: NodeId,
    pub information: Vec<Id>,
    pub assumptions: Vec<Id>,
    pub expected: String,
    pub switches: DecisionSwitch,
    // --- extensions ---
    pub action: ActionSpec,
    pub kind: DecisionKind,
    pub reversibility: Reversibility,
    pub impact: Impact,
    pub path: DecisionPath,
    pub stakes_eur: f64,
    pub novelty: f64,
    pub confidence: f64,
    pub forecasts: Vec<Id>,
    pub tier: Tier,
    pub rung_at_time: Rung,
    pub reasoning: String,
    #[serde(default)]
    pub alternatives: Vec<String>,
    #[serde(default)]
    pub premortem: Option<Id>,
    /// Minute the pre-mortem started (or would have started) and minute of commitment (§13.1).
    #[serde(default)]
    pub premortem_started_ts: Option<Ts>,
    #[serde(default)]
    pub committed_ts: Option<Ts>,
    #[serde(default)]
    pub committed_day: Option<Day>,
    #[serde(default)]
    pub challenges: Vec<Id>,
    #[serde(default)]
    pub gate: Option<GateReview>,
    #[serde(default)]
    pub approvals: Vec<Approval>,
    #[serde(default)]
    pub overrides: Vec<Override>,
    pub accountable: NodeId,
    pub status: DecisionStatus,
    #[serde(default)]
    pub outcome: Option<Outcome>,
    #[serde(default)]
    pub action_elements: ActionElements,
    #[serde(default)]
    pub commitment_point: Option<String>,
    #[serde(default)]
    pub informed: Vec<NodeId>,
    #[serde(default)]
    pub actions: Vec<Id>,
    /// Day of the earliest deviation/anomaly cited in `information` (for decision latency).
    #[serde(default)]
    pub first_signal_day: Option<Day>,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Action {
    pub id: Id,
    pub decision: Id,
    pub module: String,
    pub kind: String,
    pub params: Value,
    pub executed_day: Day,
    pub effect_summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScenarioNode {
    pub id: Id,
    #[serde(default)]
    pub parent: Vec<Id>,
    pub title: String,
    pub p: f64,
    #[serde(default)]
    pub leading_indicators: Vec<IndicatorRule>,
    #[serde(default)]
    pub assumptions: Vec<Id>,
    pub tier: ScenarioTier,
    /// "approved" | "pending" | "rejected" | "logged"
    pub status: String,
    #[serde(default)]
    pub approved_by: Option<NodeId>,
    #[serde(default)]
    pub proposed_by: Option<NodeId>,
    #[serde(default)]
    pub created: Day,
    #[serde(default)]
    pub history: Vec<ConfidencePoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClaimNode {
    pub id: Id,
    pub kind: ClaimKind,
    pub text: String,
    #[serde(default)]
    pub links: Vec<Id>,
    /// Structured attributes (e.g. `spend_share`, `applies_to`, `requires`).
    #[serde(default)]
    pub attrs: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KeyResult {
    pub metric: String,
    pub target: f64,
    #[serde(default)]
    pub current: Option<f64>,
    /// "up" (higher is better) or "down".
    #[serde(default = "default_up")]
    pub direction: String,
}
fn default_up() -> String {
    "up".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OkrState {
    pub id: Id,
    pub objective: String,
    pub key_results: Vec<KeyResult>,
    #[serde(default)]
    pub assumptions: Vec<Id>,
    pub setter: NodeId,
    #[serde(default)]
    pub flagged: Option<String>,
    #[serde(default)]
    pub flagged_day: Option<Day>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AutonomyEvidence {
    pub brier: Option<f64>,
    pub n: usize,
    pub agreement_rate: Option<f64>,
    pub threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AutonomyChange {
    pub id: Id,
    pub day: Day,
    pub class: String,
    pub from: Rung,
    pub to: Rung,
    pub evidence: AutonomyEvidence,
    /// "monthly_review" | "quarterly_review" | "off_cycle"
    pub trigger: String,
    /// "proposed" | "approved" | "rejected"
    pub status: String,
    #[serde(default)]
    pub approved_by: Option<NodeId>,
    #[serde(default)]
    pub source: Option<HumanSource>,
}

/// What Sentinel found. Serialised as `{"category": "pathogen", "name": "hubris"}`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(tag = "category", content = "name", rename_all = "snake_case")]
pub enum FindingKind {
    Pathogen(Pathogen),
    FailureMode(AiFailureMode),
    SignalClass(SentinelClass),
    /// Extra action-side rules: "pilot_limbo", "big_bang", "missing_action_elements".
    Rule(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SentinelFinding {
    pub id: Id,
    pub day: Day,
    pub kind: FindingKind,
    pub signature: String,
    /// The computed statistic(s) that triggered the finding.
    #[serde(default)]
    pub stats: BTreeMap<String, f64>,
    pub evidence: Vec<Id>,
    pub severity: Severity,
    pub status: FindingStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GridScore {
    pub quarter: String,
    pub day: Day,
    /// rows = phase (observe, think, act); cols = modality (direction..culture). 0..1 health.
    pub cells: [[f64; 5]; 3],
    /// Residual risk per cell (share of weight, sums to 100), shown moving as tools come online.
    pub residual_risk: [[f64; 5]; 3],
    pub universal_anchor: f64,
    pub culture_trap: f64,
    pub nearest_profile: String,
    pub profile_distance: BTreeMap<String, f64>,
    pub archetype: String,
    pub notes: Vec<String>,
}

/// Observation register entry (§13.11).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Anomaly {
    pub id: Id,
    pub day: Day,
    pub by: NodeId,
    pub text: String,
    pub provenance: Provenance,
    #[serde(default)]
    pub cluster: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// One stand-in (or real human) response to a queue item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HumanAction {
    pub id: Id,
    pub day: Day,
    pub ts: Ts,
    pub person: NodeId,
    pub item: Id,
    pub kind: QueueKind,
    #[serde(default)]
    pub decision: Option<Id>,
    pub verdict: HumanVerdict,
    pub reason: String,
    #[serde(default)]
    pub amendments: Option<String>,
    pub minutes_spent: f64,
    pub confidence: f64,
    pub source: HumanSource,
    /// True when the human looked at raw data before the AI summary (glass-cockpit counter).
    #[serde(default)]
    pub manual_check: bool,
}

/// A pending item in a human's queue.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QueueItem {
    pub id: Id,
    pub person: NodeId,
    pub kind: QueueKind,
    pub created_day: Day,
    #[serde(default)]
    pub decision: Option<Id>,
    pub subject: String,
    #[serde(default)]
    pub payload: Value,
    /// Blocking items (red gate, approvals) are served first.
    pub blocking: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct LlmCallRecord {
    pub tier: String,
    #[serde(default)]
    pub requested_tier: String,
    pub model: String,
    pub provider: String,
    pub purpose: String,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub cost_usd: f64,
    pub cached: bool,
    #[serde(default)]
    pub fallback: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ThinkTrace {
    pub tier: Option<Tier>,
    pub rules_fired: Vec<RuleHit>,
    #[serde(default)]
    pub rules_failed: Vec<String>,
    pub prompt_ref: Option<String>,
    pub llm_model: Option<String>,
    pub llm_request: Option<String>,
    pub llm_response: Option<String>,
    #[serde(default)]
    pub llm: Option<LlmCallRecord>,
    /// Budget fallback text, e.g. "large→small (budget)".
    #[serde(default)]
    pub fallback: Option<String>,
    pub rationale: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RunInputs {
    pub deviations: Vec<Id>,
    pub queued_alerts: Vec<Id>,
    pub messages: Vec<Id>,
    pub due_switches: Vec<Id>,
    pub assumption_flags: Vec<Id>,
    pub okr_flags: Vec<Id>,
    pub tasks: Vec<Id>,
    /// Snapshot of the metrics the module reads (its MODULE.md `inputs`).
    pub metrics: BTreeMap<String, f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RunOutputs {
    pub decisions: Vec<Id>,
    pub actions: Vec<Id>,
    pub messages: Vec<Id>,
    pub forecasts: Vec<Id>,
    pub dismissed_alerts: Vec<Id>,
    pub anomalies: Vec<Id>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ModuleRun {
    pub day: Day,
    pub module: String,
    pub inputs: RunInputs,
    pub think: ThinkTrace,
    pub outputs: RunOutputs,
    pub cost_usd: f64,
    pub tokens: u64,
    pub duration_ms: u64,
}

/// Daily computed OTA metrics (§13.9, §13.16).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DayMetrics {
    pub decisions_committed: usize,
    pub decisions_by_tier: BTreeMap<String, usize>,
    pub decision_latency_days: Option<f64>,
    pub loop_speed_days: Option<f64>,
    pub throughput_per_day: f64,
    pub learning_rate: Option<f64>,
    pub hops_to_decider: Option<f64>,
    pub real_human_share: Option<f64>,
    pub external_signal_share: Option<f64>,
    pub human_minutes: f64,
    pub alerts_routed: usize,
    pub alerts_suppressed: usize,
    pub premortems_run: usize,
    pub premortems_skipped: usize,
    pub ledger_completeness: Option<f64>,
    pub brier_30d: Option<f64>,
    pub llm_cost_usd: f64,
    pub bad_news_raised: usize,
    #[serde(default)]
    pub extra: BTreeMap<String, f64>,
}

/// Brief drafted for leadership each day.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SignalBrief {
    pub day: Day,
    pub headline: String,
    pub items: Vec<String>,
    pub tier: Tier,
    #[serde(default)]
    pub approved_by: Option<NodeId>,
    #[serde(default)]
    pub source: Option<HumanSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Finding {
    pub id: Id,
    pub day: Day,
    /// Producing tool: "double_loop" | "coherence" | "audit" | "meta_loop" | "lattice" ...
    pub tool: String,
    pub kind: String,
    pub text: String,
    #[serde(default)]
    pub refs: Vec<Id>,
    #[serde(default)]
    pub stats: BTreeMap<String, f64>,
}

/// The complete record of one simulated day: `data/days/DNNNN.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DayRecord {
    pub day: Day,
    pub date: String,
    pub weekday: String,
    pub cadences: Vec<Cadence>,
    pub tools_enabled: Vec<String>,
    pub llm_mode: String,
    pub events: Vec<EnvEvent>,
    pub signals: Vec<Signal>,
    pub kpis: BTreeMap<String, f64>,
    pub deviations: Vec<Deviation>,
    pub anomalies: Vec<Anomaly>,
    pub assumption_updates: Vec<AssumptionUpdate>,
    pub module_runs: Vec<ModuleRun>,
    pub messages: Vec<Message>,
    pub decisions: Vec<Decision>,
    /// Decisions from earlier days that changed today (sealed, approved, reverted...).
    pub decision_updates: Vec<Decision>,
    pub premortems: Vec<PreMortem>,
    pub challenges: Vec<Challenge>,
    pub forecasts_made: Vec<Forecast>,
    pub forecasts_resolved: Vec<Forecast>,
    pub actions: Vec<Action>,
    pub human_actions: Vec<HumanAction>,
    pub human_load: BTreeMap<NodeId, f64>,
    pub queue_open: BTreeMap<NodeId, usize>,
    pub autonomy_changes: Vec<AutonomyChange>,
    pub sentinel: Vec<SentinelFinding>,
    pub findings: Vec<Finding>,
    pub brief: Option<SignalBrief>,
    pub scenario_changes: Vec<ScenarioNode>,
    pub okr_flags: Vec<OkrState>,
    pub grid: Option<GridScore>,
    pub llm_calls: Vec<LlmCallRecord>,
    pub metrics: DayMetrics,
    pub log: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssumptionUpdate {
    pub assumption: Id,
    pub from: f64,
    pub to: f64,
    pub status: AssumptionStatus,
    pub reason: String,
}
