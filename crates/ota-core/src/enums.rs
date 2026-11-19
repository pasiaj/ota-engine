//! Closed vocabularies taken from the book (and from `ota.js` in ota-atlas).
//! Every enum serialises as `snake_case` strings.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

macro_rules! str_enum {
    ($(#[$m:meta])* $name:ident { $($var:ident = $s:literal),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub enum $name { $(#[serde(rename = $s)] $var),+ }
        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$var),+];
            pub fn as_str(&self) -> &'static str { match self { $($name::$var => $s),+ } }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(self.as_str()) }
        }
        impl FromStr for $name {
            type Err = String;
            fn from_str(s: &str) -> Result<Self, String> {
                let norm = s.trim().to_ascii_lowercase().replace(['-', ' '], "_");
                match norm.as_str() {
                    $($s => Ok($name::$var),)+
                    _ => Err(format!("unknown {} `{}` (expected one of: {})", stringify!($name), s,
                                     [$($s),+].join(", "))),
                }
            }
        }
    };
}

str_enum!(
    /// The three muscles of the OTA loop.
    Phase { Observe = "observe", Think = "think", Act = "act" }
);

str_enum!(
    /// The five modalities of the success grid (order = grid column order).
    Modality { Direction = "direction", Structure = "structure", Processes = "processes",
               Capability = "capability", Culture = "culture" }
);

str_enum!(
    /// The six pathogens (ch 10).
    Pathogen { Hubris = "hubris", Complacency = "complacency",
               NormalisationOfDeviance = "normalisation_of_deviance", Groupthink = "groupthink",
               MisleadingIncentives = "misleading_incentives", Inertia = "inertia" }
);

str_enum!(
    /// The 13 AI failure modes, exactly as listed in ota.js `failure_modes` (§13.1).
    AiFailureMode {
        Bypass = "bypass",
        Abdication = "abdication",
        SyntheticCertainty = "synthetic_certainty",
        AlertFatigue = "alert_fatigue",
        AiEchoChamber = "ai_echo_chamber",
        DecisionLaundering = "decision_laundering",
        AssumptionGraveyard = "assumption_graveyard",
        GlassCockpit = "glass_cockpit",
        LearningLoopInversion = "learning_loop_inversion",
        DiffusionOfAccountability = "diffusion_of_accountability",
        Homogenisation = "homogenisation",
        EpistemicCorruption = "epistemic_corruption",
        CulturalBoundary = "cultural_boundary",
    }
);

str_enum!(
    /// Sentinel signal classes in addition to pathogens (§13.2).
    SentinelClass { CalibrationDrift = "calibration_drift", CapabilityDecay = "capability_decay",
                    FallingDissent = "falling_dissent", DecayingMetrics = "decaying_metrics" }
);

str_enum!(Reversibility { Reversible = "reversible", CostlyToReverse = "costly_to_reverse", Irreversible = "irreversible" });

str_enum!(
    /// Who must be informed (ch 4).
    Impact { Local = "local", CrossFunction = "cross_function", CompanyWide = "company_wide" }
);

str_enum!(
    /// Autonomy rungs. [deviation] the book's ladder has no named rungs; these five are ours.
    Rung { Inform = "inform", Recommend = "recommend", ActWithApproval = "act_with_approval",
           ActThenReview = "act_then_review", Autonomous = "autonomous" }
);

impl Rung {
    pub fn level(self) -> u8 {
        self as u8
    }
    pub fn from_level(l: u8) -> Rung {
        Rung::ALL[(l as usize).min(4)]
    }
    /// Whether a decision at this rung executes without waiting for a human.
    pub fn executes_without_approval(self) -> bool {
        self >= Rung::ActThenReview
    }
}

str_enum!(GateColor { Green = "green", Yellow = "yellow", Red = "red" });

str_enum!(
    /// Which engine tier made a decision.
    Tier { Rule = "rule", Small = "small", Large = "large", Human = "human" }
);

str_enum!(
    /// Decision-architecture path (ch 16): Path 1 = reversible & local, AI by default.
    DecisionPath { Path1 = "path1", Path2 = "path2" }
);

str_enum!(Provenance { Internal = "internal", External = "external" });

str_enum!(
    /// Who produced a human-side record (§13.17).
    HumanSource { RealHuman = "real_human", Standin = "standin" }
);

str_enum!(NodeKind { Leader = "leader", AiModule = "ai_module", Toolkit = "toolkit", Validator = "validator",
                     EndNode = "end_node", Auditor = "auditor" });

str_enum!(Intent { Inform = "inform", Request = "request", Propose = "propose", Challenge = "challenge",
                   Escalate = "escalate", Approve = "approve", Reject = "reject", Task = "task", Report = "report" });

str_enum!(AssumptionStatus { Active = "active", Confirmed = "confirmed", Refuted = "refuted",
                             Stale = "stale", Archived = "archived" });

str_enum!(DecisionStatus { Proposed = "proposed", Approved = "approved", Rejected = "rejected",
                           Executing = "executing", Sealed = "sealed", Reverted = "reverted",
                           Blocked = "blocked", Deferred = "deferred" });

str_enum!(
    /// Scenario-tree governance tiers (§13.6).
    ScenarioTier { Major = "major", Minor = "minor", Urgent = "urgent" }
);

str_enum!(ClaimKind { Mission = "mission", Strategy = "strategy", Okr = "okr", Budget = "budget",
                      Incentive = "incentive", Policy = "policy", Regulation = "regulation" });

str_enum!(Cadence { Daily = "daily", Weekly = "weekly", Monthly = "monthly", Quarterly = "quarterly" });

str_enum!(
    /// Decision kind used by pilot-limbo / big-bang rules and the four action elements.
    DecisionKind { Routine = "routine", Test = "test", GoKill = "go_kill", Launch = "launch", Scale = "scale" }
);

str_enum!(HumanVerdict { Approve = "approve", Reject = "reject", Amend = "amend", Defer = "defer" });

str_enum!(QueueKind { Approval = "approval", GateReview = "gate_review", Review = "review", Task = "task",
                      Framing = "framing", AutonomyChange = "autonomy_change", ScenarioApproval = "scenario_approval",
                      BriefApproval = "brief_approval", MetaLoopProposal = "meta_loop_proposal" });

str_enum!(Severity { Low = "low", Medium = "medium", High = "high" });

str_enum!(FindingStatus { Open = "open", Acknowledged = "acknowledged", Resolved = "resolved" });
