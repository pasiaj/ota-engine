//! # ota-spec
//!
//! Loads a company directory — `company.md`, `constitution.md`, `toolkit.md`, `llm.toml`,
//! `modules/**/MODULE.md` with `decisions/*.md`, `prompts/*.md`, `assumptions.md`,
//! `kpis.md`, and `people/<id>/PERSONA.md` + `judgment.md` — into a validated
//! [`CompanySpec`]. The exact formats are in `docs/SPEC-FORMAT.md`.
//!
//! The loader is lenient about layout (unknown keys are ignored, keys are case- and
//! dash-insensitive) but strict about meaning: a module without an `accountable`
//! human is refused, decision-table expressions must parse, and the layered rollout
//! order in `toolkit.md` must respect tool dependencies.

pub mod loader;
pub mod md;
pub mod types;

pub use loader::{canonical_tool, load_company, parse_day, SpecErrors, TOOLS};
pub use types::*;
