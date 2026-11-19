//! Loads a company directory (see `docs/SPEC-FORMAT.md`).

use crate::md;
use crate::types::*;
use ota_core::*;
use ota_decide::{Row, Table};
use ota_llm::LlmConfig;
use serde_yaml::{Mapping, Value as Y};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// All problems found while loading; the loader keeps going to report as many as it can.
#[derive(Debug, Clone)]
pub struct SpecErrors(pub Vec<String>);

impl std::fmt::Display for SpecErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "company spec has {} error(s):", self.0.len())?;
        for e in &self.0 {
            writeln!(f, "  - {e}")?;
        }
        Ok(())
    }
}
impl std::error::Error for SpecErrors {}

/// Canonical toolkit tools and their hard dependencies (layered rollout, ch 13).
pub const TOOLS: &[(&str, &[&str])] = &[
    ("ledger", &[]),
    ("assumptions", &["ledger"]),
    ("adversarial", &["assumptions"]),
    ("premortem", &["ledger"]),
    ("lattice", &["ledger"]),
    ("anomalies", &["lattice"]),
    ("brief", &["lattice"]),
    ("scenarios", &["assumptions"]),
    ("calibration", &["ledger"]),
    ("autonomy", &["calibration"]),
    ("gate", &["ledger"]),
    ("okrs", &["assumptions"]),
    ("coherence", &["ledger"]),
    ("double_loop", &["ledger"]),
    ("sentinel", &["lattice", "calibration"]),
    ("meta_loop", &["sentinel"]),
    ("audit", &["sentinel"]),
    ("grid", &["ledger"]),
];

pub fn canonical_tool(name: &str) -> Option<&'static str> {
    let n = md::norm_key(name).replace("__", "_");
    let alias = match n.as_str() {
        "decision_ledger" => "ledger",
        "assumption_cycle" | "assumption" => "assumptions",
        "adversarial_engine" | "red_team" => "adversarial",
        "auto_premortem" | "pre_mortem" | "automatic_pre_mortem" | "auto_pre_mortem" | "automatic_premortem" => "premortem",
        "reality_lattice" => "lattice",
        "signal_brief" => "brief",
        "scenario_tree" | "living_scenario_tree" | "scenario" => "scenarios",
        "strategy_coherence" | "strategy_coherence_engine" => "coherence",
        "living_okrs" | "okr" => "okrs",
        "calibration_tracker" => "calibration",
        "autonomy_ladder" => "autonomy",
        "domain_expert_gate" => "gate",
        "double_loop_learning" | "doubleloop" => "double_loop",
        "independent_audit" => "audit",
        "success_grid" | "grid_self_assessment" => "grid",
        "anomaly_journal" | "observation_register" => "anomalies",
        "metaloop" => "meta_loop",
        other => other,
    };
    TOOLS.iter().map(|(t, _)| *t).find(|t| *t == alias)
}

/// Parse "1", "day 1", "D0061", "month 3" (= day 61), "week 2" (= day 8).
pub fn parse_day(v: &str) -> Option<Day> {
    let t = v.trim().to_ascii_lowercase();
    let t = t.trim_start_matches('d');
    if let Ok(n) = t.trim().parse::<u32>() {
        return Some(n.max(1));
    }
    let (unit, n) = t.trim().split_once(' ')?;
    let n: u32 = n.trim().split(['-', '–']).next()?.trim().parse().ok()?;
    match unit.trim().trim_start_matches("ay") {
        "" | "day" => Some(n.max(1)),
        "month" => Some((n.max(1) - 1) * 30 + 1),
        "week" => Some((n.max(1) - 1) * 7 + 1),
        "quarter" => Some((n.max(1) - 1) * 91 + 1),
        _ => None,
    }
}

struct Ctx {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl Ctx {
    fn err(&mut self, p: &Path, m: impl Into<String>) {
        self.errors.push(format!("{}: {}", p.display(), m.into()));
    }
    fn warn(&mut self, p: &Path, m: impl Into<String>) {
        self.warnings.push(format!("{}: {}", p.display(), m.into()));
    }
    fn parse<T: std::str::FromStr<Err = String>>(&mut self, p: &Path, v: Option<String>, d: T) -> T {
        match v {
            None => d,
            Some(s) if s.trim().is_empty() => d,
            Some(s) => match s.parse() {
                Ok(x) => x,
                Err(e) => {
                    self.err(p, e);
                    d
                }
            },
        }
    }
}

// ---------------------------------------------------------------- YAML helpers

struct Fm(Mapping);

impl Fm {
    fn load(p: &Path, yaml: Option<String>, cx: &mut Ctx) -> Fm {
        match yaml {
            None => Fm(Mapping::new()),
            Some(y) => match serde_yaml::from_str::<Y>(&y) {
                Ok(Y::Mapping(m)) => Fm(m),
                Ok(Y::Null) => Fm(Mapping::new()),
                Ok(_) => {
                    cx.err(p, "front-matter is not a mapping");
                    Fm(Mapping::new())
                }
                Err(e) => {
                    cx.err(p, format!("invalid YAML front-matter: {e}"));
                    Fm(Mapping::new())
                }
            },
        }
    }
    fn raw(&self, keys: &[&str]) -> Option<&Y> {
        for (k, v) in &self.0 {
            if let Some(ks) = k.as_str() {
                if keys.iter().any(|x| md::norm_key(ks) == *x) {
                    return Some(v);
                }
            }
        }
        None
    }
    fn s(&self, keys: &[&str]) -> Option<String> {
        self.raw(keys).and_then(yaml_str)
    }
    fn list(&self, keys: &[&str]) -> Vec<String> {
        self.raw(keys).map(yaml_list).unwrap_or_default()
    }
    fn f(&self, keys: &[&str]) -> Option<f64> {
        self.raw(keys).and_then(|v| v.as_f64().or_else(|| yaml_str(v).and_then(|s| s.trim().parse().ok())))
    }
    fn b(&self, keys: &[&str]) -> Option<bool> {
        self.raw(keys).and_then(|v| v.as_bool().or_else(|| yaml_str(v).and_then(|s| md::parse_bool(&s))))
    }
}

fn yaml_str(v: &Y) -> Option<String> {
    match v {
        Y::String(s) => Some(s.clone()),
        Y::Number(n) => Some(n.to_string()),
        Y::Bool(b) => Some(b.to_string()),
        Y::Sequence(s) => Some(s.iter().filter_map(yaml_str).collect::<Vec<_>>().join(", ")),
        _ => None,
    }
}

fn yaml_list(v: &Y) -> Vec<String> {
    match v {
        Y::Sequence(s) => s.iter().filter_map(yaml_str).collect(),
        Y::Null => vec![],
        other => yaml_str(other).map(|s| md::list(&s)).unwrap_or_default(),
    }
}

fn yaml_to_json(v: &Y) -> serde_json::Value {
    serde_json::to_value(v).unwrap_or(serde_json::Value::Null)
}

fn read(p: &Path, cx: &mut Ctx) -> Option<String> {
    match fs::read_to_string(p) {
        Ok(s) => Some(s),
        Err(e) => {
            cx.err(p, format!("cannot read: {e}"));
            None
        }
    }
}

fn first_heading(body: &str) -> Option<String> {
    body.lines().find_map(|l| l.trim().strip_prefix("# ").map(|s| s.trim().to_string()))
}

fn bullet_map(src: &str) -> BTreeMap<String, String> {
    md::bullets(src).into_iter().filter(|(k, _)| !k.is_empty()).collect()
}

fn opt(m: &BTreeMap<String, String>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|k| m.get(*k)).filter(|s| !s.trim().is_empty()).cloned()
}

// ---------------------------------------------------------------- entry point

/// Load and validate a company directory.
pub fn load_company(root: impl AsRef<Path>) -> Result<CompanySpec, SpecErrors> {
    let root = root.as_ref().to_path_buf();
    let mut cx = Ctx { errors: vec![], warnings: vec![] };
    if !root.is_dir() {
        return Err(SpecErrors(vec![format!("{}: not a directory", root.display())]));
    }
    let company = load_company_md(&root, &mut cx);
    let constitution = load_constitution(&root, &mut cx);
    let toolkit = load_toolkit(&root, &mut cx);
    let llm = load_llm(&root, &mut cx);
    let people = load_people(&root, &mut cx);
    let mut modules = vec![];
    let mut classes = BTreeMap::new();
    let mut assumptions = vec![];
    let mut kpis = vec![];
    let mdir = root.join("modules");
    let mut module_files = vec![];
    find_files(&mdir, "MODULE.md", &mut module_files);
    module_files.sort();
    if module_files.is_empty() {
        cx.warn(&mdir, "no modules found (expected modules/**/MODULE.md)");
    }
    for f in module_files {
        if let Some(m) = load_module(&f, &people, &mut cx) {
            for c in load_classes(&m, &people, &mut cx) {
                if classes.contains_key(&c.id) {
                    cx.err(&c.file, format!("duplicate decision class `{}`", c.id));
                }
                classes.insert(c.id.clone(), c);
            }
            let ap = m.dir.join("assumptions.md");
            if ap.exists() {
                assumptions.extend(load_assumptions(&ap, Some(&m.id), &m.id, &mut cx));
            }
            let kp = m.dir.join("kpis.md");
            if kp.exists() {
                kpis.extend(load_kpis(&kp, Some(&m.id), &mut cx));
            }
            modules.push(m);
        }
    }
    // classes owned by another module than their directory
    for c in classes.values() {
        if let Some(m) = modules.iter_mut().find(|m| m.id == c.module) {
            if !m.classes.contains(&c.id) {
                m.classes.push(c.id.clone());
            }
        } else {
            cx.err(&c.file, format!("decision class `{}` names owner module `{}` which does not exist", c.id, c.module));
        }
    }
    let default_owner = people.iter().find(|p| p.role == NodeKind::Leader).map(|p| p.id.clone()).unwrap_or_default();
    let rp = root.join("assumptions.md");
    if rp.exists() {
        assumptions.extend(load_assumptions(&rp, None, &default_owner, &mut cx));
    }
    let kp = root.join("kpis.md");
    if kp.exists() {
        kpis.extend(load_kpis(&kp, None, &mut cx));
    }
    let scenarios = opt_file(&root, "scenarios.md").map(|p| load_scenarios(&p, &mut cx)).unwrap_or_default();
    let okrs = opt_file(&root, "okrs.md").map(|p| load_okrs(&p, &default_owner, &mut cx)).unwrap_or_default();
    let claims = opt_file(&root, "strategy.md").map(|p| load_claims(&p, &mut cx)).unwrap_or_default();

    let spec = CompanySpec { root: root.clone(), company, constitution, toolkit, llm, modules, people, classes,
                             assumptions, kpis, scenarios, okrs, claims, warnings: vec![] };
    cross_validate(&spec, &mut cx);
    if !cx.errors.is_empty() {
        return Err(SpecErrors(cx.errors));
    }
    Ok(CompanySpec { warnings: cx.warnings, ..spec })
}

fn opt_file(root: &Path, name: &str) -> Option<PathBuf> {
    let p = root.join(name);
    p.exists().then_some(p)
}

fn find_files(dir: &Path, name: &str, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            find_files(&p, name, out);
        } else if p.file_name().and_then(|s| s.to_str()) == Some(name) {
            out.push(p);
        }
    }
}

fn md_files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir).map(|rd| rd.flatten().map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("md")).collect()).unwrap_or_default();
    v.sort();
    v
}

// ---------------------------------------------------------------- company-level files

fn load_company_md(root: &Path, cx: &mut Ctx) -> CompanyInfo {
    let p = root.join("company.md");
    let (fm, body) = match fs::read_to_string(&p) {
        Ok(s) => {
            let (y, b) = md::front_matter(&s);
            (Fm::load(&p, y, cx), b)
        }
        Err(_) => {
            cx.err(&p, "missing company.md");
            (Fm(Mapping::new()), String::new())
        }
    };
    let dir_name = root.file_name().and_then(|s| s.to_str()).unwrap_or("company").to_string();
    let start_date = fm.s(&["start_date", "day1", "day_1"]).unwrap_or_else(|| "2025-01-01".into());
    if chrono_like_date(&start_date).is_none() {
        cx.err(&p, format!("start_date `{start_date}` is not YYYY-MM-DD"));
    }
    let mut extra = BTreeMap::new();
    for (k, v) in &fm.0 {
        if let Some(k) = k.as_str() {
            extra.insert(k.to_string(), yaml_to_json(v));
        }
    }
    CompanyInfo {
        id: fm.s(&["id"]).unwrap_or(dir_name),
        name: fm.s(&["name"]).or_else(|| first_heading(&body)).unwrap_or_else(|| "Company".into()),
        start_date,
        timezone: fm.s(&["timezone", "tz"]).unwrap_or_else(|| "Europe/Helsinki".into()),
        currency: fm.s(&["currency"]).unwrap_or_else(|| "EUR".into()),
        premortem_threshold: fm.f(&["premortem_threshold"]).unwrap_or(1000.0),
        alert_budget: fm.f(&["alert_budget"]).map(|x| x as usize).unwrap_or(5),
        learning_curve_kpis: fm.list(&["learning_curve_kpis"]),
        pilot_limbo_n: fm.f(&["pilot_limbo_n"]).map(|x| x as usize).unwrap_or(3),
        description: body.trim().to_string(),
        extra,
    }
}

/// Returns (y, m, d) for a YYYY-MM-DD string.
pub fn chrono_like_date(s: &str) -> Option<(i32, u32, u32)> {
    let mut it = s.trim().split('-');
    let y = it.next()?.parse().ok()?;
    let m: u32 = it.next()?.parse().ok()?;
    let d: u32 = it.next()?.parse().ok()?;
    ((1..=12).contains(&m) && (1..=31).contains(&d)).then_some((y, m, d))
}

fn load_constitution(root: &Path, cx: &mut Ctx) -> Vec<Principle> {
    let p = root.join("constitution.md");
    let Ok(s) = fs::read_to_string(&p) else {
        cx.warn(&p, "missing constitution.md");
        return vec![];
    };
    let (_, body) = md::front_matter(&s);
    let items = md::numbered(&body);
    if items.is_empty() {
        cx.warn(&p, "no numbered principles found (expected `1. ...`)");
    }
    items.into_iter().enumerate().map(|(i, t)| {
        let lower = t.to_ascii_lowercase();
        let (text, enforced_by) = match lower.find("enforced by:") {
            Some(ix) => (t[..ix].trim().trim_end_matches(['—', '-', '(', ' ']).trim().to_string(),
                         Some(t[ix + 12..].trim().trim_end_matches(')').trim().to_string())),
            None => (t.clone(), None),
        };
        Principle { n: i + 1, text, enforced_by }
    }).collect()
}

fn load_toolkit(root: &Path, cx: &mut Ctx) -> Vec<ToolSpec> {
    let p = root.join("toolkit.md");
    let mut found: BTreeMap<&'static str, ToolSpec> = BTreeMap::new();
    if let Ok(s) = fs::read_to_string(&p) {
        let (y, body) = md::front_matter(&s);
        let fm = Fm::load(&p, y, cx);
        if let Some(Y::Mapping(tools)) = fm.raw(&["tools"]) {
            for (k, v) in tools {
                let name = k.as_str().unwrap_or_default();
                let Some(id) = canonical_tool(name) else {
                    cx.err(&p, format!("unknown tool `{name}` (known: {})", TOOLS.iter().map(|t| t.0).collect::<Vec<_>>().join(", ")));
                    continue;
                };
                let (from, evolves, check) = match v {
                    Y::Mapping(m) => {
                        let f = Fm(m.clone());
                        (f.s(&["enabled_from"]), f.s(&["evolves_from"]), f.s(&["human_checkpoint"]))
                    }
                    other => (yaml_str(other), None, None),
                };
                let day = from.as_deref().and_then(parse_day);
                if day.is_none() {
                    cx.err(&p, format!("tool `{name}`: enabled_from `{}` is not a day (use 1, \"day 61\", \"month 3\")", from.unwrap_or_default()));
                }
                found.insert(id, ToolSpec { id: id.into(), enabled_from: day.unwrap_or(1), depends_on: vec![], evolves_from: evolves, human_checkpoint: check });
            }
        }
        for table in md::tables(&body) {
            for row in table {
                let Some(name) = row.get("tool").or_else(|| row.get("id")) else { continue };
                let name = md::unquote(name);
                let Some(id) = canonical_tool(&name) else {
                    cx.err(&p, format!("unknown tool `{name}` in table"));
                    continue;
                };
                let from = row.get("enabled_from").or_else(|| row.get("enabled")).or_else(|| row.get("from")).cloned().unwrap_or_default();
                let day = parse_day(&md::unquote(&from));
                if day.is_none() {
                    cx.err(&p, format!("tool `{name}`: enabled_from `{from}` is not a day"));
                }
                found.insert(id, ToolSpec {
                    id: id.into(),
                    enabled_from: day.unwrap_or(1),
                    depends_on: vec![],
                    evolves_from: row.get("evolves_from").filter(|s| !s.is_empty()).cloned(),
                    human_checkpoint: row.get("human_checkpoint").filter(|s| !s.is_empty()).cloned(),
                });
            }
        }
    } else {
        cx.warn(&p, "missing toolkit.md: every tool is enabled from day 1");
    }
    let mut missing = vec![];
    let mut out = vec![];
    for (id, deps) in TOOLS {
        let mut t = found.remove(id).unwrap_or_else(|| {
            missing.push(*id);
            ToolSpec { id: id.to_string(), enabled_from: 1, depends_on: vec![], evolves_from: None, human_checkpoint: None }
        });
        t.depends_on = deps.iter().map(|s| s.to_string()).collect();
        out.push(t);
    }
    if !missing.is_empty() && p.exists() {
        cx.warn(&p, format!("tools not listed, enabled from day 1: {}", missing.join(", ")));
    }
    // layered rollout: a tool cannot be enabled before its dependencies
    for t in &out {
        for d in &t.depends_on {
            let dep = out.iter().find(|x| &x.id == d).unwrap();
            if t.enabled_from < dep.enabled_from {
                cx.err(&p, format!("layered rollout violated: `{}` enabled from day {} but depends on `{}` enabled from day {}",
                                   t.id, t.enabled_from, d, dep.enabled_from));
            }
        }
    }
    out
}

fn load_llm(root: &Path, cx: &mut Ctx) -> LlmConfig {
    let p = root.join("llm.toml");
    match fs::read_to_string(&p) {
        Ok(s) => LlmConfig::parse(&s).unwrap_or_else(|e| {
            cx.err(&p, format!("invalid llm.toml: {e}"));
            LlmConfig::default()
        }),
        Err(_) => {
            cx.warn(&p, "missing llm.toml: using default tiers");
            LlmConfig::default()
        }
    }
}

// ---------------------------------------------------------------- people

fn load_people(root: &Path, cx: &mut Ctx) -> Vec<PersonSpec> {
    let dir = root.join("people");
    let mut files = vec![];
    find_files(&dir, "PERSONA.md", &mut files);
    files.sort();
    let mut out: Vec<PersonSpec> = vec![];
    for p in files {
        let Some(s) = read(&p, cx) else { continue };
        let (y, body) = md::front_matter(&s);
        let fm = Fm::load(&p, y, cx);
        let pdir = p.parent().unwrap().to_path_buf();
        let id = fm.s(&["id"]).unwrap_or_else(|| pdir.file_name().unwrap().to_string_lossy().into());
        let role = cx.parse(&p, fm.s(&["role", "kind", "role_kind"]), NodeKind::EndNode);
        if matches!(role, NodeKind::AiModule | NodeKind::Toolkit) {
            cx.err(&p, format!("person `{id}` has role `{role}`; people must be leader, validator, end_node or auditor"));
        }
        let bias_src = match fm.raw(&["bias", "bias_profile"]) {
            Some(Y::Mapping(m)) => Fm(m.clone()),
            _ => Fm(fm.0.clone()),
        };
        let bias = Bias {
            overconfidence: bias_src.f(&["overconfidence"]).unwrap_or(0.1),
            challenge_acceptance: bias_src.f(&["challenge_acceptance"]).unwrap_or(0.6),
            review_diligence: bias_src.f(&["review_diligence"]).unwrap_or(0.7),
            extra: match fm.raw(&["bias", "bias_profile"]) {
                Some(Y::Mapping(m)) => m.iter().filter_map(|(k, v)| {
                    let k = md::norm_key(k.as_str()?);
                    (!matches!(k.as_str(), "overconfidence" | "challenge_acceptance" | "review_diligence")).then(|| (k, yaml_to_json(v)))
                }).collect(),
                _ => BTreeMap::new(),
            },
        };
        let mut gate_rules = vec![];
        if let Some(Y::Sequence(rules)) = fm.raw(&["gate_rules", "gate"]) {
            for r in rules {
                if let Y::Mapping(m) = r {
                    let g = Fm(m.clone());
                    let rule = GateRule { classes: g.list(&["classes", "class"]), red_if: g.s(&["red_if", "red"]), yellow_if: g.s(&["yellow_if", "yellow"]) };
                    for e in [&rule.red_if, &rule.yellow_if].into_iter().flatten() {
                        if let Err(er) = ota_decide::parse(e) {
                            cx.err(&p, format!("gate rule expression `{e}`: {er}"));
                        }
                    }
                    if rule.classes.is_empty() {
                        cx.err(&p, "gate rule without `classes` (use [\"*\"] for all)");
                    }
                    gate_rules.push(rule);
                }
            }
        }
        let judgment = fs::read_to_string(pdir.join("judgment.md")).unwrap_or_else(|_| {
            cx.warn(&p, format!("person `{id}` has no judgment.md"));
            String::new()
        });
        if out.iter().any(|x| x.id == id) {
            cx.err(&p, format!("duplicate person id `{id}`"));
        }
        out.push(PersonSpec {
            name: fm.s(&["name"]).or_else(|| first_heading(&body)).unwrap_or_else(|| id.clone()),
            title: fm.s(&["title"]).unwrap_or_default(),
            accountable_for: fm.list(&["accountable_for", "accountable"]),
            expertise: fm.list(&["expertise", "expertise_domains", "domains"]),
            llm_tier: fm.s(&["llm_tier", "tier"]).unwrap_or_else(|| "human".into()),
            weekly_capacity_min: fm.f(&["weekly_capacity_min", "weekly_capacity", "capacity_min"]).unwrap_or(600.0),
            shadow_group: fm.b(&["shadow_group"]).unwrap_or(role == NodeKind::EndNode),
            bias,
            gate_rules,
            persona: body.trim().to_string(),
            judgment,
            memory: fs::read_to_string(pdir.join("memory.md")).ok(),
            dir: pdir,
            role,
            id,
        });
    }
    out
}

// ---------------------------------------------------------------- modules

fn load_module(p: &Path, people: &[PersonSpec], cx: &mut Ctx) -> Option<ModuleSpec> {
    let s = read(p, cx)?;
    let (y, body) = md::front_matter(&s);
    if y.is_none() {
        cx.err(p, "MODULE.md has no YAML front-matter (--- id: ... ---)");
    }
    let fm = Fm::load(p, y, cx);
    let dir = p.parent().unwrap().to_path_buf();
    let id = fm.s(&["id"]).unwrap_or_else(|| dir.file_name().unwrap().to_string_lossy().into());
    let org = fm.s(&["org", "organisation", "organization"])
        .unwrap_or_else(|| dir.parent().and_then(|d| d.file_name()).map(|s| s.to_string_lossy().into()).unwrap_or_default());
    let Some(accountable) = fm.s(&["accountable"]).filter(|s| !s.trim().is_empty()) else {
        cx.err(p, format!("module `{id}` names no `accountable` human (constitution principle 6); refusing to load it"));
        return None;
    };
    if !people.iter().any(|x| x.id == accountable) {
        cx.err(p, format!("module `{id}`: accountable `{accountable}` is not a person under people/"));
    }
    let mut phases = vec![];
    for ph in fm.list(&["phases", "phase"]) {
        phases.push(cx.parse(p, Some(ph), Phase::Think));
    }
    if phases.is_empty() {
        phases.push(Phase::Think);
    }
    let tool = fm.s(&["tool"]).map(|t| match canonical_tool(&t) {
        Some(c) => c.to_string(),
        None => {
            cx.warn(p, format!("module `{id}`: unknown tool `{t}`"));
            t
        }
    });
    let kind_default = if tool.is_some() { NodeKind::Toolkit } else { NodeKind::AiModule };
    let mut prompts = BTreeMap::new();
    for f in md_files(&dir.join("prompts")) {
        if let Ok(t) = fs::read_to_string(&f) {
            prompts.insert(f.file_stem().unwrap().to_string_lossy().into(), t);
        }
    }
    Some(ModuleSpec {
        name: fm.s(&["name"]).or_else(|| first_heading(&body)).unwrap_or_else(|| id.clone()),
        org,
        kind: cx.parse(p, fm.s(&["kind", "node_kind"]), kind_default),
        phases,
        cadence: cx.parse(p, fm.s(&["cadence"]).map(|c| c.split([':', ' ']).next().unwrap_or("").to_string()), Cadence::Daily),
        inputs: fm.list(&["inputs"]),
        outputs: fm.list(&["outputs"]),
        owner: fm.s(&["owner"]),
        accountable,
        evolves_from: fm.s(&["evolves_from"]),
        depends_on: fm.list(&["depends_on", "depends"]),
        alert_budget: fm.f(&["alert_budget"]).map(|x| x as usize),
        modality: fm.s(&["modality"]).map(|m| cx.parse(p, Some(m), Modality::Processes)),
        tool,
        metric_owner_of: fm.list(&["metric_owner_of", "metric_owner"]),
        body: body.trim().to_string(),
        playbook: fs::read_to_string(dir.join("playbook.md")).ok(),
        classes: vec![],
        prompts,
        dir,
        id,
    })
}

fn load_classes(m: &ModuleSpec, people: &[PersonSpec], cx: &mut Ctx) -> Vec<DecisionClass> {
    let mut out = vec![];
    for f in md_files(&m.dir.join("decisions")) {
        let Some(s) = read(&f, cx) else { continue };
        let (y, body) = md::front_matter(&s);
        let fm = Fm::load(&f, y, cx);
        let secs: Vec<md::Section> = md::sections(&body, 2).into_iter()
            .filter(|s| s.title.to_ascii_lowercase().starts_with("decision class")).collect();
        let stem = f.file_stem().unwrap().to_string_lossy().to_string();
        if secs.is_empty() {
            let id = fm.s(&["id", "class"]).unwrap_or_else(|| format!("{}.{}", m.id, stem));
            if let Some(c) = build_class(&id, &fm, &body, m, &f, people, cx) {
                out.push(c);
            }
        } else {
            for sec in secs {
                let id = sec.title.split_once(':').map(|(_, r)| md::unquote(r.trim())).filter(|s| !s.is_empty())
                    .unwrap_or_else(|| format!("{}.{}", m.id, stem));
                if let Some(c) = build_class(&id, &fm, &sec.body, m, &f, people, cx) {
                    out.push(c);
                }
            }
        }
    }
    out
}

fn build_class(id: &str, fm: &Fm, body: &str, m: &ModuleSpec, f: &Path, people: &[PersonSpec], cx: &mut Ctx) -> Option<DecisionClass> {
    let mut meta = bullet_map(body.split("\n|").next().unwrap_or(body));
    // front-matter keys are defaults; bullets win
    for (k, v) in &fm.0 {
        if let (Some(k), Some(v)) = (k.as_str(), yaml_str(v)) {
            meta.entry(md::norm_key(k)).or_insert(v);
        }
    }
    let g = |k: &[&str]| opt(&meta, k);
    let table = md::tables(body).into_iter().find(|t| t.first().map(|r| r.contains_key("when") && r.contains_key("then")).unwrap_or(false));
    let Some(table) = table else {
        cx.err(f, format!("decision class `{id}` has no decision table with `when` and `then` columns"));
        return None;
    };
    let mut rows = vec![];
    for (i, r) in table.iter().enumerate() {
        let conf = r.get("confidence").map(|s| md::unquote(s)).unwrap_or_default();
        let confidence = if conf.is_empty() { 0.5 } else {
            conf.trim_end_matches('%').parse::<f64>().map(|x| if conf.ends_with('%') { x / 100.0 } else { x }).unwrap_or_else(|_| {
                cx.err(f, format!("class `{id}` row {}: confidence `{conf}` is not a number", i + 1));
                0.5
            })
        };
        let esc = r.get("escalate").map(|s| md::unquote(s)).unwrap_or_default();
        let escalate = match esc.trim().to_ascii_lowercase().as_str() {
            "" | "-" | "—" | "no" | "none" => None,
            other => Some(cx.parse(f, Some(other.to_string()), Tier::Small)),
        };
        rows.push(Row {
            when: md::unquote(r.get("when").map(String::as_str).unwrap_or("")),
            then: md::unquote(r.get("then").map(String::as_str).unwrap_or("")),
            confidence,
            escalate,
            note: r.get("note").or_else(|| r.get("why")).filter(|s| !s.is_empty()).cloned(),
        });
    }
    if let Err((i, e)) = Table::compile(id, &rows) {
        cx.err(f, format!("class `{id}` row {} (`{}` → `{}`): {e}", i + 1, rows[i].when, rows[i].then));
    }
    for k in ["confirm_if", "stop_if", "forecast_if"] {
        if let Some(e) = g(&[k]) {
            if let Err(er) = ota_decide::parse(&e) {
                cx.err(f, format!("class `{id}` {k} `{e}`: {er}"));
            }
        }
    }
    let module = g(&["owner", "module"]).unwrap_or_else(|| m.id.clone());
    let accountable = g(&["accountable"]).unwrap_or_else(|| m.accountable.clone());
    if !people.iter().any(|p| p.id == accountable) {
        cx.err(f, format!("class `{id}`: accountable `{accountable}` is not a person under people/"));
    }
    let reversibility = cx.parse(f, g(&["reversibility"]), Reversibility::Reversible);
    let impact = cx.parse(f, g(&["impact"]), Impact::Local);
    let path = cx.parse(f, g(&["path"]), if reversibility == Reversibility::Reversible && impact == Impact::Local { DecisionPath::Path1 } else { DecisionPath::Path2 });
    let rung_default = match (path, reversibility) {
        (DecisionPath::Path1, _) => Rung::ActThenReview,
        (_, Reversibility::Irreversible) => Rung::Recommend,
        _ => Rung::ActWithApproval,
    };
    let num = |cx: &mut Ctx, k: &str, d: f64| -> f64 {
        match g(&[k]) {
            None => d,
            Some(v) => v.replace(['_', ' ', '€'], "").parse().unwrap_or_else(|_| {
                cx.err(f, format!("class `{id}`: `{k}` = `{v}` is not a number"));
                d
            }),
        }
    };
    let known: &[&str] = &["owner", "module", "accountable", "reversibility", "impact", "path", "default_rung", "rung", "stakes_eur", "stakes",
        "novelty", "kind", "modality", "human_only", "validator", "assumptions", "forecast", "forecast_if", "forecast_horizon_days",
        "confirm_if", "stop_if", "check_after_days", "hypothesis", "learning_metric", "decision_point", "commitment_point",
        "record_holds", "description", "id", "class"];
    let extra = meta.iter().filter(|(k, _)| !known.contains(&k.as_str())).map(|(k, v)| (k.clone(), v.clone())).collect();
    let stakes = if meta.contains_key("stakes_eur") { num(cx, "stakes_eur", 0.0) } else { num(cx, "stakes", 0.0) };
    Some(DecisionClass {
        id: id.to_string(),
        module,
        accountable,
        reversibility,
        impact,
        path,
        default_rung: cx.parse(f, g(&["default_rung", "rung"]), rung_default),
        stakes_eur: stakes,
        novelty: num(cx, "novelty", 0.2),
        kind: cx.parse(f, g(&["kind"]), DecisionKind::Routine),
        modality: g(&["modality"]).map(|v| cx.parse(f, Some(v), Modality::Processes)),
        human_only: g(&["human_only"]).and_then(|v| md::parse_bool(&v)).unwrap_or(false),
        validator: g(&["validator"]),
        assumptions: g(&["assumptions"]).map(|v| md::list(&v)).unwrap_or_default(),
        forecast: g(&["forecast"]),
        forecast_if: g(&["forecast_if"]),
        forecast_horizon_days: num(cx, "forecast_horizon_days", 7.0) as u32,
        confirm_if: g(&["confirm_if"]),
        stop_if: g(&["stop_if"]),
        check_after_days: num(cx, "check_after_days", 7.0) as u32,
        hypothesis: g(&["hypothesis"]),
        learning_metric: g(&["learning_metric"]),
        decision_point: g(&["decision_point"]),
        commitment_point: g(&["commitment_point"]),
        record_holds: g(&["record_holds"]).and_then(|v| md::parse_bool(&v)).unwrap_or(false),
        description: body.lines().filter(|l| !l.trim_start().starts_with(['-', '*', '|', '#'])).collect::<Vec<_>>().join("\n").trim().to_string(),
        rows,
        file: f.to_path_buf(),
        extra,
    })
}

// ---------------------------------------------------------------- assumptions, KPIs, scenarios, OKRs, claims

fn indicators(meta: &[(String, String)], p: &Path, cx: &mut Ctx) -> Vec<IndicatorRule> {
    let mut v = vec![];
    for (k, e) in meta {
        let effect = match k.as_str() {
            "confirm" | "confirm_if" | "confirms" => "confirm",
            "refute" | "refute_if" | "refutes" => "refute",
            _ => continue,
        };
        if let Err(er) = ota_decide::parse(e) {
            cx.err(p, format!("indicator `{e}`: {er}"));
        }
        v.push(IndicatorRule { expr: e.clone(), effect: effect.into() });
    }
    v
}

fn load_assumptions(p: &Path, module: Option<&str>, default_owner: &str, cx: &mut Ctx) -> Vec<AssumptionSpec> {
    let Some(s) = read(p, cx) else { return vec![] };
    let (_, body) = md::front_matter(&s);
    let mut out = vec![];
    for sec in md::sections(&body, 2).into_iter().filter(|s| !s.title.is_empty()) {
        let (id, title_text) = match sec.title.split_once(':') {
            Some((a, b)) => (md::unquote(a), b.trim().to_string()),
            None => (md::unquote(&sec.title), String::new()),
        };
        let bl = md::bullets(&sec.body);
        let m: BTreeMap<String, String> = bl.iter().filter(|(k, _)| !k.is_empty()).cloned().collect();
        let text = opt(&m, &["text", "statement"]).unwrap_or_else(|| if title_text.is_empty() {
            sec.body.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with(['-', '*'])).unwrap_or("").to_string()
        } else { title_text.clone() });
        out.push(AssumptionSpec {
            id: id.clone(),
            text,
            owner: opt(&m, &["owner"]).unwrap_or_else(|| default_owner.to_string()),
            module: module.map(String::from),
            confidence: opt(&m, &["confidence"]).and_then(|v| v.parse().ok()).unwrap_or(0.6),
            load_bearing: opt(&m, &["load_bearing"]).and_then(|v| md::parse_bool(&v)).unwrap_or(false),
            indicators: indicators(&bl, p, cx),
            depends: opt(&m, &["depends", "decisions"]).map(|v| md::list(&v)).unwrap_or_default(),
        });
    }
    for t in md::tables(&body) {
        for r in t {
            let Some(id) = r.get("id").cloned() else { continue };
            let mut bl = vec![];
            if let Some(c) = r.get("confirm").filter(|s| !s.is_empty()) { bl.push(("confirm".to_string(), md::unquote(c))); }
            if let Some(c) = r.get("refute").filter(|s| !s.is_empty()) { bl.push(("refute".to_string(), md::unquote(c))); }
            out.push(AssumptionSpec {
                id,
                text: r.get("text").or_else(|| r.get("assumption")).cloned().unwrap_or_default(),
                owner: r.get("owner").filter(|s| !s.is_empty()).cloned().unwrap_or_else(|| default_owner.to_string()),
                module: module.map(String::from),
                confidence: r.get("confidence").and_then(|v| v.parse().ok()).unwrap_or(0.6),
                load_bearing: r.get("load_bearing").and_then(|v| md::parse_bool(v)).unwrap_or(false),
                indicators: indicators(&bl, p, cx),
                depends: vec![],
            });
        }
    }
    out
}

fn load_kpis(p: &Path, module: Option<&str>, cx: &mut Ctx) -> Vec<KpiSpec> {
    let Some(s) = read(p, cx) else { return vec![] };
    let mut out = vec![];
    for t in md::tables(&s) {
        for r in t {
            let Some(metric) = r.get("metric").or_else(|| r.get("kpi")).map(|s| md::unquote(s)) else { continue };
            let target = r.get("target").map(|s| md::unquote(s)).filter(|s| !s.is_empty() && s != "-");
            let tnum = target.as_ref().and_then(|t| t.replace(['_', ' ', '€', '$'], "").trim_end_matches('%').parse::<f64>().ok()
                .map(|x| if t.ends_with('%') { x / 100.0 } else { x }));
            if target.is_some() && tnum.is_none() {
                cx.warn(p, format!("kpi `{metric}`: target `{}` is not numeric; ignored", target.unwrap()));
            }
            out.push(KpiSpec {
                metric,
                target: tnum,
                direction: r.get("direction").filter(|s| !s.is_empty()).cloned().unwrap_or_else(|| "up".into()),
                unit: r.get("unit").cloned().unwrap_or_default(),
                owner: r.get("owner").filter(|s| !s.is_empty()).cloned().or_else(|| module.map(String::from)),
                description: r.get("description").cloned().unwrap_or_default(),
                domain: r.get("domain").filter(|s| !s.is_empty()).cloned(),
            });
        }
    }
    out
}

fn load_scenarios(p: &Path, cx: &mut Ctx) -> Vec<ScenarioNode> {
    let Some(s) = read(p, cx) else { return vec![] };
    let mut out = vec![];
    for sec in md::sections(&s, 2).into_iter().filter(|s| !s.title.is_empty()) {
        let (id, title) = sec.title.split_once(':').map(|(a, b)| (md::unquote(a), b.trim().to_string()))
            .unwrap_or_else(|| (md::unquote(&sec.title), sec.title.clone()));
        let bl = md::bullets(&sec.body);
        let m: BTreeMap<String, String> = bl.iter().filter(|(k, _)| !k.is_empty()).cloned().collect();
        out.push(ScenarioNode {
            id,
            parent: opt(&m, &["parent", "parents"]).map(|v| md::list(&v)).unwrap_or_default(),
            title,
            p: opt(&m, &["p", "probability"]).and_then(|v| v.parse().ok()).unwrap_or(0.5),
            leading_indicators: indicators(&bl, p, cx),
            assumptions: opt(&m, &["assumptions"]).map(|v| md::list(&v)).unwrap_or_default(),
            tier: cx.parse(p, opt(&m, &["tier"]), ScenarioTier::Minor),
            status: opt(&m, &["status"]).unwrap_or_else(|| "approved".into()),
            approved_by: opt(&m, &["approved_by"]),
            proposed_by: opt(&m, &["proposed_by"]),
            created: 0,
            history: vec![],
        });
    }
    out
}

fn load_okrs(p: &Path, default_setter: &str, cx: &mut Ctx) -> Vec<OkrState> {
    let Some(s) = read(p, cx) else { return vec![] };
    let mut out = vec![];
    for sec in md::sections(&s, 2).into_iter().filter(|s| !s.title.is_empty()) {
        let (id, objective) = sec.title.split_once(':').map(|(a, b)| (md::unquote(a), b.trim().to_string()))
            .unwrap_or_else(|| (md::unquote(&sec.title), sec.title.clone()));
        let bl = md::bullets(&sec.body);
        let m: BTreeMap<String, String> = bl.iter().filter(|(k, _)| !k.is_empty()).cloned().collect();
        let mut krs = vec![];
        for (k, v) in &bl {
            if k != "kr" && k != "key_result" { continue; }
            let ops = [">=", "<=", "≥", "≤", ">", "<"];
            match ops.iter().find_map(|o| v.split_once(o).map(|(a, b)| (o, a, b))) {
                Some((o, a, b)) => match b.trim().replace('_', "").parse::<f64>() {
                    Ok(t) => krs.push(KeyResult { metric: a.trim().to_string(), target: t, current: None,
                        direction: if matches!(*o, ">=" | "≥" | ">") { "up".into() } else { "down".into() } }),
                    Err(_) => cx.err(p, format!("OKR `{id}`: key result `{v}` target is not a number")),
                },
                None => cx.err(p, format!("OKR `{id}`: key result `{v}` must look like `metric >= target`")),
            }
        }
        out.push(OkrState {
            id,
            objective,
            key_results: krs,
            assumptions: opt(&m, &["assumptions"]).map(|v| md::list(&v)).unwrap_or_default(),
            setter: opt(&m, &["setter", "owner"]).unwrap_or_else(|| default_setter.to_string()),
            flagged: None,
            flagged_day: None,
        });
    }
    out
}

fn load_claims(p: &Path, cx: &mut Ctx) -> Vec<ClaimNode> {
    let Some(s) = read(p, cx) else { return vec![] };
    let mut out = vec![];
    for sec in md::sections(&s, 2).into_iter().filter(|s| !s.title.is_empty()) {
        let (id, title) = sec.title.split_once(':').map(|(a, b)| (md::unquote(a), b.trim().to_string()))
            .unwrap_or_else(|| (md::unquote(&sec.title), String::new()));
        let m = bullet_map(&sec.body);
        let kind = cx.parse(p, opt(&m, &["kind"]), ClaimKind::Strategy);
        let mut attrs = BTreeMap::new();
        for (k, v) in &m {
            if matches!(k.as_str(), "kind" | "text" | "links") { continue; }
            let val = if v.starts_with('[') { serde_json::json!(md::list(v)) }
                      else if let Ok(x) = v.parse::<f64>() { serde_json::json!(x) }
                      else { serde_json::json!(v) };
            attrs.insert(k.clone(), val);
        }
        out.push(ClaimNode {
            id,
            kind,
            text: opt(&m, &["text"]).unwrap_or(title),
            links: opt(&m, &["links"]).map(|v| md::list(&v)).unwrap_or_default(),
            attrs,
        });
    }
    out
}

// ---------------------------------------------------------------- cross validation

fn cross_validate(s: &CompanySpec, cx: &mut Ctx) {
    let ids: Vec<&str> = s.modules.iter().map(|m| m.id.as_str()).collect();
    for m in &s.modules {
        if ids.iter().filter(|x| **x == m.id).count() > 1 {
            cx.err(&m.dir, format!("duplicate module id `{}`", m.id));
        }
        for d in &m.depends_on {
            if !ids.contains(&d.as_str()) {
                cx.warn(&m.dir, format!("module `{}` depends_on unknown module `{d}`", m.id));
            }
        }
    }
    let assumption_ids: Vec<&str> = s.assumptions.iter().map(|a| a.id.as_str()).collect();
    for c in s.classes.values() {
        if let Some(v) = &c.validator {
            match s.person(v) {
                None => cx.err(&c.file, format!("class `{}`: validator `{v}` is not a person", c.id)),
                Some(p) if p.role != NodeKind::Validator => cx.warn(&c.file, format!("class `{}`: validator `{v}` has role {}", c.id, p.role)),
                _ => {}
            }
        }
        for a in &c.assumptions {
            if !assumption_ids.contains(&a.as_str()) {
                cx.warn(&c.file, format!("class `{}` rests on unknown assumption `{a}`", c.id));
            }
        }
    }
    for p in &s.people {
        for c in &p.accountable_for {
            if !s.classes.contains_key(c) && !c.ends_with('*') && !ids.contains(&c.as_str()) {
                cx.warn(&p.dir, format!("{} is accountable_for unknown class or module `{c}`", p.id));
            }
        }
    }
    if !s.people.iter().any(|p| p.role == NodeKind::Leader) {
        cx.err(&s.root, "no person with role `leader` under people/");
    }
    let a: Vec<&str> = assumption_ids.clone();
    for (i, x) in a.iter().enumerate() {
        if a[..i].contains(x) {
            cx.err(&s.root, format!("duplicate assumption id `{x}`"));
        }
    }
}
