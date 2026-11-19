//! # ota-decide — the Decisions API (§5)
//!
//! Decision tables are evaluated deterministically, top to bottom; the first row whose
//! `when` is true wins (`otherwise` always matches). A row may ask to `escalate` to an
//! LLM tier or a human. The expression language is in [`expr`].

pub mod expr;
pub mod table;

pub use expr::{bindings, eval_bool, eval_str, parse, Context, EvalError, Expr};
pub use table::{is_otherwise, parse_then, parse_then_shape, Row, Table, TableEval};

#[cfg(test)]
mod tests {
    use super::*;
    use ota_core::Tier;
    use std::collections::BTreeMap;

    struct Ctx(BTreeMap<String, f64>);
    impl Context for Ctx {
        fn var(&self, n: &str) -> Option<f64> { self.0.get(n).copied() }
        fn z(&self, m: &str) -> Option<f64> { self.0.get(&format!("z:{m}")).copied() }
        fn trend(&self, m: &str, d: u32) -> Option<f64> { self.0.get(m).map(|v| v / d as f64) }
        fn days_since(&self, n: &str) -> Option<f64> { if n == "launch" { Some(12.0) } else { None } }
    }
    fn ctx() -> Ctx {
        let mut m = BTreeMap::new();
        for (k, v) in [("roas_d7", 1.3), ("target", 1.0), ("spend_util", 0.95), ("cpi", 2.0), ("z:cpi", 3.5), ("kc2.dau", 1000.0)] {
            m.insert(k.to_string(), v);
        }
        Ctx(m)
    }

    #[test]
    fn arithmetic_and_precedence() {
        let c = ctx();
        assert_eq!(eval_str("1 + 2 * 3", &c).unwrap(), 7.0);
        assert_eq!(eval_str("(1 + 2) * 3", &c).unwrap(), 9.0);
        assert_eq!(eval_str("-2 * -3", &c).unwrap(), 6.0);
        assert_eq!(eval_str("10 / 4", &c).unwrap(), 2.5);
        assert_eq!(eval_str("15%", &c).unwrap(), 0.15);
        assert_eq!(eval_str("kc2.dau / 1_000", &c).unwrap(), 1.0);
    }

    #[test]
    fn logic_and_comparisons() {
        let c = ctx();
        assert!(eval_bool("roas_d7 > target * 1.15 && spend_util > 0.9", &c).unwrap());
        assert!(!eval_bool("roas_d7 < target * 0.8", &c).unwrap());
        assert!(eval_bool("!(cpi > 3) || false", &c).unwrap());
        assert!(eval_bool("cpi >= 2 and cpi <= 2", &c).unwrap());
        assert!(eval_bool("cpi ≥ 2", &c).unwrap());
        assert!(eval_bool("cpi == 2 && cpi != 3", &c).unwrap());
        // short circuit: unknown on the right is never evaluated
        assert!(eval_bool("true || nope > 1", &c).unwrap());
    }

    #[test]
    fn functions() {
        let c = ctx();
        assert_eq!(eval_str("min(3, cpi, 5)", &c).unwrap(), 2.0);
        assert_eq!(eval_str("max(3, cpi, 5)", &c).unwrap(), 5.0);
        assert_eq!(eval_str("abs(-4)", &c).unwrap(), 4.0);
        assert!(eval_bool("z(cpi) > 3", &c).unwrap());
        assert_eq!(eval_str("trend(cpi, 4)", &c).unwrap(), 0.5);
        assert_eq!(eval_str("days_since(launch)", &c).unwrap(), 12.0);
        assert_eq!(eval_str("days_since(\"launch\")", &c).unwrap(), 12.0);
        assert!(matches!(eval_str("days_since(never)", &c), Err(EvalError::Unknown(_))));
        assert!(matches!(eval_str("foo(1)", &c), Err(EvalError::UnknownFunction(_))));
    }

    #[test]
    fn errors() {
        let c = ctx();
        assert!(matches!(eval_str("missing + 1", &c), Err(EvalError::Unknown(n)) if n == "missing"));
        assert!(parse("1 +").is_err());
        assert!(parse("(1").is_err());
        assert!(parse("1 2").is_err());
        assert!(parse("").is_err());
    }

    fn row(w: &str, t: &str, c: f64, e: Option<Tier>) -> Row {
        Row { when: w.into(), then: t.into(), confidence: c, escalate: e, note: None }
    }

    #[test]
    fn table_first_match_and_otherwise() {
        let rows = vec![
            row("roas_d7 > target * 1.15 && spend_util > 0.9", "bid *= 1.10", 0.72, None),
            row("roas_d7 < target * 0.80", "bid *= 0.85", 0.70, None),
            row("z(cpi) > 3", "hold", 0.40, Some(Tier::Small)),
            row("otherwise", "hold", 0.6, None),
        ];
        let t = Table::compile("ua.bid_adjust", &rows).unwrap();
        let c = ctx();
        let ev = t.evaluate(&c);
        let hit = ev.hit.unwrap();
        assert_eq!(hit.row, 0);
        assert_eq!(hit.bindings.get("roas_d7"), Some(&1.3));
        let a = parse_then(&hit.then, &c).unwrap();
        assert_eq!(a.kind, "adjust");
        assert_eq!(a.params["op"], "*");
        assert!((a.params["value"].as_f64().unwrap() - 1.1).abs() < 1e-9);
        assert_eq!(t.metrics(), vec!["roas_d7", "target", "spend_util", "cpi"]);

        let mut c2 = ctx();
        c2.0.insert("roas_d7".into(), 0.9);
        let hit = t.evaluate(&c2).hit.unwrap();
        assert_eq!(hit.row, 2);
        assert_eq!(hit.escalate, Some(Tier::Small));
        c2.0.insert("z:cpi".into(), 0.0);
        assert_eq!(t.evaluate(&c2).hit.unwrap().row, 3);
    }

    #[test]
    fn missing_metric_is_not_a_match() {
        let rows = vec![row("unknown_metric > 1", "go", 0.9, None), row("otherwise", "hold", 0.5, None)];
        let t = Table::compile("x", &rows).unwrap();
        let ev = t.evaluate(&ctx());
        assert_eq!(ev.hit.unwrap().row, 1);
        assert_eq!(ev.errors.len(), 1);
    }

    #[test]
    fn then_forms() {
        let c = ctx();
        assert!(parse_then("hold", &c).unwrap().is_hold());
        assert!(parse_then("", &c).unwrap().is_hold());
        let a = parse_then("shift_budget(from=meta, to=\"applovin\", amount=cpi * 100)", &c).unwrap();
        assert_eq!(a.kind, "shift_budget");
        assert_eq!(a.params["from"], "meta");
        assert_eq!(a.params["to"], "applovin");
        assert_eq!(a.params["amount"], 200.0);
        let a = parse_then("kill", &c).unwrap();
        assert_eq!(a.kind, "kill");
        let a = parse_then("price = cpi + 1", &c).unwrap();
        assert_eq!(a.params["op"], "set");
        assert_eq!(a.params["value"], 3.0);
        let a = parse_then("launch(0.5)", &c).unwrap();
        assert_eq!(a.params["value"], 0.5);
        assert!(Table::compile("bad", &[row("x >", "hold", 0.5, None)]).is_err());
        assert!(Table::compile("bad", &[row("x > 1", "go(", 0.5, None)]).is_err());
    }
}
