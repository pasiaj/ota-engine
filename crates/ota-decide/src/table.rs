//! Decision tables: first matching row wins; `otherwise` matches always.

use crate::expr::{self, Context, EvalError, Expr};
use ota_core::{ActionSpec, RuleHit, Tier};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Row {
    pub when: String,
    pub then: String,
    pub confidence: f64,
    #[serde(default)]
    pub escalate: Option<Tier>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone)]
struct Compiled {
    row: Row,
    cond: Option<Expr>, // None = otherwise
}

#[derive(Debug, Clone)]
pub struct Table {
    pub class: String,
    rows: Vec<Compiled>,
}

#[derive(Debug, Clone, Default)]
pub struct TableEval {
    pub hit: Option<RuleHit>,
    /// Rows whose condition could not be evaluated (missing metric, ...). Not a match.
    pub errors: Vec<String>,
}

pub fn is_otherwise(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "otherwise" | "else" | "*" | "default")
}

impl Table {
    /// Compile all rows; returns (row index, error) for the first row that does not parse.
    pub fn compile(class: &str, rows: &[Row]) -> Result<Table, (usize, EvalError)> {
        let mut out = vec![];
        for (i, r) in rows.iter().enumerate() {
            let cond = if is_otherwise(&r.when) { None } else { Some(expr::parse(&r.when).map_err(|e| (i, e))?) };
            parse_then_shape(&r.then).map_err(|e| (i, e))?;
            out.push(Compiled { row: r.clone(), cond });
        }
        Ok(Table { class: class.to_string(), rows: out })
    }

    pub fn rows(&self) -> impl Iterator<Item = &Row> {
        self.rows.iter().map(|c| &c.row)
    }

    /// Metric names referenced by any row.
    pub fn metrics(&self) -> Vec<String> {
        let mut v = vec![];
        for c in &self.rows {
            if let Some(e) = &c.cond {
                for m in e.metrics() {
                    if !v.contains(&m) { v.push(m); }
                }
            }
        }
        v
    }

    pub fn evaluate(&self, ctx: &dyn Context) -> TableEval {
        let mut ev = TableEval::default();
        for (i, c) in self.rows.iter().enumerate() {
            let matched = match &c.cond {
                None => true,
                Some(e) => match e.truthy(ctx) {
                    Ok(m) => m,
                    Err(err) => {
                        ev.errors.push(format!("{} row {}: {}", self.class, i + 1, err));
                        false
                    }
                },
            };
            if matched {
                ev.hit = Some(RuleHit {
                    class: self.class.clone(),
                    row: i,
                    when: c.row.when.clone(),
                    then: c.row.then.clone(),
                    confidence: c.row.confidence,
                    escalate: c.row.escalate,
                    bindings: c.cond.as_ref().map(|e| expr::bindings(e, ctx)).unwrap_or_default(),
                });
                break;
            }
        }
        ev
    }
}

/// Parse a `then` cell into an action:
/// * `hold`, `none`, `-`, empty → hold
/// * `target *= expr` / `+=` / `-=` / `=` → `{kind:"adjust", params:{target, op, value}}`
/// * `kind(k = expr, k2 = "text")` or `kind(expr)` → `{kind, params:{k: value, ...}}`
/// * `kind` → `{kind, params:{}}`
pub fn parse_then(src: &str, ctx: &dyn Context) -> Result<ActionSpec, EvalError> {
    let s = src.trim().trim_matches('`').trim();
    if s.is_empty() || matches!(s.to_ascii_lowercase().as_str(), "hold" | "none" | "-" | "noop" | "—") {
        return Ok(ActionSpec::hold());
    }
    for op in ["*=", "+=", "-=", "/="] {
        if let Some((t, v)) = s.split_once(op) {
            let value = expr::eval_str(v, ctx)?;
            return Ok(ActionSpec {
                kind: "adjust".into(),
                params: json!({"target": t.trim(), "op": op.trim_end_matches('='), "value": value}),
            });
        }
    }
    if let Some((t, v)) = s.split_once('=') {
        if !t.contains('(') && !v.starts_with('=') {
            let value = expr::eval_str(v, ctx)?;
            return Ok(ActionSpec { kind: "adjust".into(), params: json!({"target": t.trim(), "op": "set", "value": value}) });
        }
    }
    if let Some(open) = s.find('(') {
        if !s.ends_with(')') {
            return Err(EvalError::Parse(format!("action `{s}` missing closing `)`")));
        }
        let kind = s[..open].trim().to_string();
        let inner = &s[open + 1..s.len() - 1];
        let mut params = Map::new();
        for (i, part) in split_args(inner).into_iter().enumerate() {
            let part = part.trim();
            if part.is_empty() { continue; }
            let (k, v) = match part.split_once('=') {
                Some((k, v)) if !v.starts_with('=') && !k.contains(['<', '>', '!']) => (k.trim().to_string(), v.trim()),
                _ => (if i == 0 { "value".to_string() } else { format!("arg{i}") }, part),
            };
            params.insert(k, value_of(v, ctx)?);
        }
        return Ok(ActionSpec { kind, params: Value::Object(params) });
    }
    if s.chars().all(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '-')) {
        return Ok(ActionSpec { kind: s.to_string(), params: json!({}) });
    }
    Err(EvalError::Parse(format!("cannot parse action `{s}`")))
}

fn value_of(v: &str, ctx: &dyn Context) -> Result<Value, EvalError> {
    let t = v.trim();
    if (t.starts_with('"') && t.ends_with('"') && t.len() >= 2) || (t.starts_with('\'') && t.ends_with('\'') && t.len() >= 2) {
        return Ok(Value::String(t[1..t.len() - 1].to_string()));
    }
    match expr::eval_str(t, ctx) {
        Ok(x) => Ok(json!(x)),
        // bare words are allowed as string values: `launch(stage=soft)`
        Err(EvalError::Unknown(_)) if t.chars().all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.')) => {
            Ok(Value::String(t.to_string()))
        }
        Err(e) => Err(e),
    }
}

fn split_args(s: &str) -> Vec<String> {
    let (mut out, mut cur, mut depth, mut q) = (vec![], String::new(), 0i32, None::<char>);
    for c in s.chars() {
        match (c, q) {
            ('"' | '\'', None) => { q = Some(c); cur.push(c); }
            (c2, Some(qq)) if c2 == qq => { q = None; cur.push(c); }
            ('(', None) => { depth += 1; cur.push(c); }
            (')', None) => { depth -= 1; cur.push(c); }
            (',', None) if depth == 0 => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out
}

/// Syntax-only check of a `then` cell at load time (no context needed).
pub fn parse_then_shape(src: &str) -> Result<(), EvalError> {
    struct Any;
    impl Context for Any {
        fn var(&self, _: &str) -> Option<f64> { Some(1.0) }
        fn z(&self, _: &str) -> Option<f64> { Some(1.0) }
        fn trend(&self, _: &str, _: u32) -> Option<f64> { Some(1.0) }
        fn days_since(&self, _: &str) -> Option<f64> { Some(1.0) }
        fn mean(&self, _: &str, _: u32) -> Option<f64> { Some(1.0) }
        fn prev(&self, _: &str, _: u32) -> Option<f64> { Some(1.0) }
    }
    parse_then(src, &Any).map(|_| ())
}
