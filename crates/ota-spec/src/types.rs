//! Loaded, validated company specification.

use ota_core::*;
use ota_decide::Row;
use ota_llm::LlmConfig;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyInfo {
    pub id: String,
    pub name: String,
    /// Calendar date of simulation day 1, `YYYY-MM-DD`.
    pub start_date: String,
    pub timezone: String,
    pub currency: String,
    /// Pre-mortem runs when stakes_eur × novelty ≥ this.
    pub premortem_threshold: f64,
    /// Default per-node daily alert budget.
    pub alert_budget: usize,
    /// KPIs whose cost should follow a Wright learning curve (learning-rate metric).
    pub learning_curve_kpis: Vec<String>,
    /// Pilot limbo: more than this many open tests without go/kill in a module.
    pub pilot_limbo_n: usize,
    /// Days of history before day 1 the backstory covers (informational).
    pub description: String,
    #[serde(default)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Principle {
    pub n: usize,
    pub text: String,
    pub enforced_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub id: String,
    pub enabled_from: Day,
    pub depends_on: Vec<String>,
    pub evolves_from: Option<String>,
    pub human_checkpoint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleSpec {
    pub id: String,
    pub name: String,
    pub org: String,
    pub kind: NodeKind,
    pub phases: Vec<Phase>,
    pub cadence: Cadence,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub owner: Option<String>,
    pub accountable: String,
    pub evolves_from: Option<String>,
    pub depends_on: Vec<String>,
    pub alert_budget: Option<usize>,
    pub modality: Option<Modality>,
    /// For toolkit modules: the engine tool they represent (`lattice`, `ledger`, ...).
    pub tool: Option<String>,
    /// Strategic signal domains this module is the metric owner of (§13.10).
    pub metric_owner_of: Vec<String>,
    pub body: String,
    pub dir: PathBuf,
    pub classes: Vec<String>,
    pub prompts: BTreeMap<String, String>,
    pub playbook: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionClass {
    pub id: String,
    pub module: String,
    pub accountable: String,
    pub reversibility: Reversibility,
    pub impact: Impact,
    pub path: DecisionPath,
    pub default_rung: Rung,
    pub stakes_eur: f64,
    pub novelty: f64,
    pub kind: DecisionKind,
    pub modality: Option<Modality>,
    pub human_only: bool,
    pub validator: Option<String>,
    pub assumptions: Vec<String>,
    pub forecast: Option<String>,
    pub forecast_if: Option<String>,
    pub forecast_horizon_days: u32,
    pub confirm_if: Option<String>,
    pub stop_if: Option<String>,
    pub check_after_days: u32,
    pub hypothesis: Option<String>,
    pub learning_metric: Option<String>,
    pub decision_point: Option<String>,
    pub commitment_point: Option<String>,
    pub record_holds: bool,
    pub description: String,
    pub rows: Vec<Row>,
    pub file: PathBuf,
    #[serde(default)]
    pub extra: BTreeMap<String, String>,
}

impl DecisionClass {
    pub fn requires_action_elements(&self) -> bool {
        matches!(self.kind, DecisionKind::Test | DecisionKind::Launch | DecisionKind::Scale | DecisionKind::GoKill)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Bias {
    pub overconfidence: f64,
    pub challenge_acceptance: f64,
    pub review_diligence: f64,
    /// Any further bias keys the company defines (passed to stand-ins verbatim).
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateRule {
    /// Decision classes (or `*`) the rule covers.
    pub classes: Vec<String>,
    pub red_if: Option<String>,
    pub yellow_if: Option<String>,
}

impl GateRule {
    pub fn covers(&self, class: &str) -> bool {
        self.classes.iter().any(|c| c == "*" || c == class || (c.ends_with(".*") && class.starts_with(&c[..c.len() - 1])))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonSpec {
    pub id: String,
    pub name: String,
    pub role: NodeKind,
    pub title: String,
    pub accountable_for: Vec<String>,
    pub expertise: Vec<String>,
    pub llm_tier: String,
    pub weekly_capacity_min: f64,
    pub bias: Bias,
    pub gate_rules: Vec<GateRule>,
    pub shadow_group: bool,
    pub persona: String,
    pub judgment: String,
    pub memory: Option<String>,
    pub dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KpiSpec {
    pub metric: String,
    pub target: Option<f64>,
    /// "up" | "down" | "band"
    pub direction: String,
    pub unit: String,
    pub owner: Option<String>,
    pub description: String,
    /// Signal domain for the truth pipeline (ua_cost, platform_policy, ...).
    pub domain: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssumptionSpec {
    pub id: String,
    pub text: String,
    pub owner: String,
    pub module: Option<String>,
    pub confidence: f64,
    pub load_bearing: bool,
    pub indicators: Vec<IndicatorRule>,
    pub depends: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanySpec {
    pub root: PathBuf,
    pub company: CompanyInfo,
    pub constitution: Vec<Principle>,
    pub toolkit: Vec<ToolSpec>,
    pub llm: LlmConfig,
    pub modules: Vec<ModuleSpec>,
    pub people: Vec<PersonSpec>,
    pub classes: BTreeMap<String, DecisionClass>,
    pub assumptions: Vec<AssumptionSpec>,
    pub kpis: Vec<KpiSpec>,
    pub scenarios: Vec<ScenarioNode>,
    pub okrs: Vec<OkrState>,
    pub claims: Vec<ClaimNode>,
    pub warnings: Vec<String>,
}

impl CompanySpec {
    pub fn module(&self, id: &str) -> Option<&ModuleSpec> {
        self.modules.iter().find(|m| m.id == id)
    }
    pub fn person(&self, id: &str) -> Option<&PersonSpec> {
        self.people.iter().find(|p| p.id == id)
    }
    pub fn leaders(&self) -> impl Iterator<Item = &PersonSpec> {
        self.people.iter().filter(|p| p.role == NodeKind::Leader)
    }
    pub fn tool(&self, id: &str) -> Option<&ToolSpec> {
        self.toolkit.iter().find(|t| t.id == id)
    }
    /// Whether `tool` is switched on for `day` (layered rollout, §13.7).
    pub fn tool_enabled(&self, tool: &str, day: Day) -> bool {
        self.tool(tool).map(|t| day >= t.enabled_from).unwrap_or(false)
    }
    pub fn kpi(&self, m: &str) -> Option<&KpiSpec> {
        self.kpis.iter().find(|k| k.metric == m)
    }
    pub fn is_human(&self, id: &str) -> bool {
        self.person(id).is_some()
    }
}
