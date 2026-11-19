use ota_core::*;
use std::path::PathBuf;

fn tiny() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/tiny/company")
}

#[test]
fn loads_the_tiny_company() {
    let s = ota_spec::load_company(tiny()).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(s.company.name, "Tiny Games Oy");
    assert_eq!(s.constitution.len(), 7);
    assert_eq!(s.modules.len(), 2);
    assert_eq!(s.people.len(), 2);
    assert_eq!(s.classes.len(), 4);
    let c = &s.classes["ua.bid_adjust"];
    assert_eq!(c.module, "ua");
    assert_eq!(c.path, DecisionPath::Path1);
    assert_eq!(c.default_rung, Rung::ActThenReview);
    assert_eq!(c.rows.len(), 4);
    assert_eq!(c.rows[2].escalate, Some(Tier::Small));
    let b = &s.classes["ua.budget_shift"];
    assert_eq!(b.path, DecisionPath::Path2);
    assert_eq!(b.default_rung, Rung::ActWithApproval);
    assert_eq!(b.kind, DecisionKind::Scale);
    assert!(s.tool_enabled("ledger", 1));
    assert!(!s.tool_enabled("sentinel", 9));
    assert_eq!(s.tool("meta_loop").unwrap().enabled_from, 12);
    assert_eq!(s.assumptions.len(), 2);
    assert_eq!(s.okrs[0].key_results.len(), 2);
    assert_eq!(s.scenarios.len(), 2);
    assert_eq!(s.claims.iter().filter(|c| c.kind == ClaimKind::Regulation).count(), 1);
    assert_eq!(s.person("ua-scientist").unwrap().gate_rules.len(), 1);
    assert_eq!(s.llm.tiers["small"].model, "gpt-4.1-nano");
    assert!(s.kpis.iter().any(|k| k.metric == "cpi" && k.owner.as_deref() == Some("ua")));
}

fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let p = e.path();
        let t = to.join(e.file_name());
        if p.is_dir() { copy_dir(&p, &t) } else { std::fs::copy(&p, &t).unwrap(); }
    }
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ota-spec-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    copy_dir(&tiny(), &d);
    d
}

#[test]
fn refuses_module_without_accountable() {
    let d = scratch("noacc");
    let p = d.join("modules/growth/ua/MODULE.md");
    let s = std::fs::read_to_string(&p).unwrap().replace("accountable: ceo\n", "");
    std::fs::write(&p, s).unwrap();
    let err = ota_spec::load_company(&d).unwrap_err().to_string();
    assert!(err.contains("names no `accountable` human"), "{err}");
}

#[test]
fn reports_bad_expressions_and_rollout_order() {
    let d = scratch("bad");
    let p = d.join("modules/growth/ua/decisions/bid_adjust.md");
    let s = std::fs::read_to_string(&p).unwrap().replace("`roas_d7 < 0.80`", "`roas_d7 < `");
    std::fs::write(&p, s).unwrap();
    let t = d.join("toolkit.md");
    let s = std::fs::read_to_string(&t).unwrap().replace("| sentinel    | 10 ", "| sentinel    | 2  ");
    std::fs::write(&t, s).unwrap();
    let err = ota_spec::load_company(&d).unwrap_err().to_string();
    assert!(err.contains("ua.bid_adjust` row 2"), "{err}");
    assert!(err.contains("layered rollout violated: `sentinel`"), "{err}");
}

#[test]
fn parse_day_forms() {
    assert_eq!(ota_spec::parse_day("1"), Some(1));
    assert_eq!(ota_spec::parse_day("day 61"), Some(61));
    assert_eq!(ota_spec::parse_day("month 3"), Some(61));
    assert_eq!(ota_spec::parse_day("D0010"), Some(10));
    assert_eq!(ota_spec::parse_day("week 2"), Some(8));
    assert_eq!(ota_spec::parse_day("soon"), None);
}
