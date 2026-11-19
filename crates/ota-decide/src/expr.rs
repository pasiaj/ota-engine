//! A small, deterministic expression language for decision tables (§5).
//!
//! ```text
//! expr    := or
//! or      := and (("||" | "or") and)*
//! and     := not (("&&" | "and") not)*
//! not     := ("!" | "not") not | cmp
//! cmp     := add (("<" | "<=" | ">" | ">=" | "==" | "!=") add)?
//! add     := mul (("+" | "-") mul)*
//! mul     := unary (("*" | "/") unary)*
//! unary   := "-" unary | primary
//! primary := NUMBER ["%"] | STRING | "true" | "false" | IDENT | IDENT "(" args ")" | "(" expr ")"
//! IDENT   := [A-Za-z_][A-Za-z0-9_.:]*
//! ```
//!
//! Values are `f64`; booleans are 1.0 / 0.0 and any non-zero value is truthy.
//! Built-in functions: `min(a, b, ...)`, `max(a, b, ...)`, `abs(x)`, `clamp(x, lo, hi)`;
//! history functions resolved by the [`Context`]: `z(metric)`, `trend(metric[, days])`,
//! `days_since(name)`, `mean(metric, days)`, `prev(metric[, days])`.

use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Num(f64),
    Str(String),
    Ident(String),
    Neg(Box<Expr>),
    Not(Box<Expr>),
    Bin(Box<Expr>, Op, Box<Expr>),
    Call(String, Vec<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Op {
    Add, Sub, Mul, Div, Lt, Le, Gt, Ge, Eq, Ne, And, Or,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EvalError {
    Parse(String),
    Unknown(String),
    UnknownFunction(String),
    Arity(String),
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvalError::Parse(m) => write!(f, "parse error: {m}"),
            EvalError::Unknown(n) => write!(f, "unknown identifier `{n}`"),
            EvalError::UnknownFunction(n) => write!(f, "unknown function `{n}`"),
            EvalError::Arity(m) => write!(f, "{m}"),
        }
    }
}
impl std::error::Error for EvalError {}

/// What an expression can see. Implemented by the runtime over KPIs + lattice history.
pub trait Context {
    fn var(&self, name: &str) -> Option<f64>;
    fn z(&self, _metric: &str) -> Option<f64> { None }
    /// Relative slope per day over the last `days` (e.g. -0.02 = falling 2%/day).
    fn trend(&self, _metric: &str, _days: u32) -> Option<f64> { None }
    fn days_since(&self, _name: &str) -> Option<f64> { None }
    fn mean(&self, _metric: &str, _days: u32) -> Option<f64> { None }
    fn prev(&self, _metric: &str, _days: u32) -> Option<f64> { None }
}

/// A plain map context (tests, simple callers).
impl Context for BTreeMap<String, f64> {
    fn var(&self, name: &str) -> Option<f64> {
        self.get(name).copied()
    }
}

// ---------------------------------------------------------------- lexer

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Str(String),
    Ident(String),
    Op(&'static str),
    LParen,
    RParen,
    Comma,
}

fn lex(src: &str) -> Result<Vec<Tok>, EvalError> {
    let s: Vec<char> = src.chars().collect();
    let mut i = 0;
    let mut out = vec![];
    while i < s.len() {
        let c = s[i];
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() || (c == '.' && i + 1 < s.len() && s[i + 1].is_ascii_digit()) {
            let st = i;
            while i < s.len() && (s[i].is_ascii_digit() || s[i] == '.' || s[i] == '_') {
                i += 1;
            }
            // exponent
            if i < s.len() && (s[i] == 'e' || s[i] == 'E') && i + 1 < s.len()
                && (s[i + 1].is_ascii_digit() || s[i + 1] == '-' || s[i + 1] == '+') {
                i += 2;
                while i < s.len() && s[i].is_ascii_digit() { i += 1; }
            }
            let txt: String = s[st..i].iter().filter(|c| **c != '_').collect();
            let mut v: f64 = txt.parse().map_err(|_| EvalError::Parse(format!("bad number `{txt}`")))?;
            if i < s.len() && s[i] == '%' {
                v /= 100.0;
                i += 1;
            }
            out.push(Tok::Num(v));
        } else if c.is_alphabetic() || c == '_' {
            let st = i;
            while i < s.len() && (s[i].is_alphanumeric() || matches!(s[i], '_' | '.' | ':')) {
                i += 1;
            }
            let w: String = s[st..i].iter().collect();
            match w.as_str() {
                "and" => out.push(Tok::Op("&&")),
                "or" => out.push(Tok::Op("||")),
                "not" => out.push(Tok::Op("!")),
                _ => out.push(Tok::Ident(w)),
            }
        } else if c == '"' || c == '\'' {
            let q = c;
            i += 1;
            let st = i;
            while i < s.len() && s[i] != q { i += 1; }
            if i >= s.len() { return Err(EvalError::Parse("unterminated string".into())); }
            out.push(Tok::Str(s[st..i].iter().collect()));
            i += 1;
        } else {
            let two: String = s[i..(i + 2).min(s.len())].iter().collect();
            let op2 = ["&&", "||", "<=", ">=", "==", "!=", "≥", "≤"].iter().find(|o| two.starts_with(**o));
            if let Some(o) = op2 {
                let o: &'static str = match *o { "≥" => ">=", "≤" => "<=", x => x };
                out.push(Tok::Op(o));
                i += if two.starts_with('≥') || two.starts_with('≤') { 1 } else { 2 };
                continue;
            }
            let t = match c {
                '(' => Tok::LParen,
                ')' => Tok::RParen,
                ',' => Tok::Comma,
                '+' => Tok::Op("+"),
                '-' | '−' => Tok::Op("-"),
                '*' | '×' => Tok::Op("*"),
                '/' => Tok::Op("/"),
                '<' => Tok::Op("<"),
                '>' => Tok::Op(">"),
                '!' => Tok::Op("!"),
                '=' => Tok::Op("=="),
                _ => return Err(EvalError::Parse(format!("unexpected character `{c}`"))),
            };
            out.push(t);
            i += 1;
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------- parser

struct P {
    t: Vec<Tok>,
    i: usize,
}

impl P {
    fn peek_op(&self, ops: &[&str]) -> Option<&'static str> {
        match self.t.get(self.i) {
            Some(Tok::Op(o)) if ops.contains(o) => Some(o),
            _ => None,
        }
    }
    fn or(&mut self) -> Result<Expr, EvalError> {
        let mut l = self.and()?;
        while self.peek_op(&["||"]).is_some() {
            self.i += 1;
            l = Expr::Bin(Box::new(l), Op::Or, Box::new(self.and()?));
        }
        Ok(l)
    }
    fn and(&mut self) -> Result<Expr, EvalError> {
        let mut l = self.not()?;
        while self.peek_op(&["&&"]).is_some() {
            self.i += 1;
            l = Expr::Bin(Box::new(l), Op::And, Box::new(self.not()?));
        }
        Ok(l)
    }
    fn not(&mut self) -> Result<Expr, EvalError> {
        if self.peek_op(&["!"]).is_some() {
            self.i += 1;
            return Ok(Expr::Not(Box::new(self.not()?)));
        }
        self.cmp()
    }
    fn cmp(&mut self) -> Result<Expr, EvalError> {
        let l = self.add()?;
        if let Some(o) = self.peek_op(&["<", "<=", ">", ">=", "==", "!="]) {
            self.i += 1;
            let op = match o { "<" => Op::Lt, "<=" => Op::Le, ">" => Op::Gt, ">=" => Op::Ge, "==" => Op::Eq, _ => Op::Ne };
            return Ok(Expr::Bin(Box::new(l), op, Box::new(self.add()?)));
        }
        Ok(l)
    }
    fn add(&mut self) -> Result<Expr, EvalError> {
        let mut l = self.mul()?;
        while let Some(o) = self.peek_op(&["+", "-"]) {
            self.i += 1;
            let op = if o == "+" { Op::Add } else { Op::Sub };
            l = Expr::Bin(Box::new(l), op, Box::new(self.mul()?));
        }
        Ok(l)
    }
    fn mul(&mut self) -> Result<Expr, EvalError> {
        let mut l = self.unary()?;
        while let Some(o) = self.peek_op(&["*", "/"]) {
            self.i += 1;
            let op = if o == "*" { Op::Mul } else { Op::Div };
            l = Expr::Bin(Box::new(l), op, Box::new(self.unary()?));
        }
        Ok(l)
    }
    fn unary(&mut self) -> Result<Expr, EvalError> {
        if self.peek_op(&["-"]).is_some() {
            self.i += 1;
            return Ok(Expr::Neg(Box::new(self.unary()?)));
        }
        self.primary()
    }
    fn primary(&mut self) -> Result<Expr, EvalError> {
        let t = self.t.get(self.i).cloned().ok_or_else(|| EvalError::Parse("unexpected end of expression".into()))?;
        self.i += 1;
        match t {
            Tok::Num(v) => Ok(Expr::Num(v)),
            Tok::Str(s) => Ok(Expr::Str(s)),
            Tok::Ident(n) if n == "true" => Ok(Expr::Num(1.0)),
            Tok::Ident(n) if n == "false" => Ok(Expr::Num(0.0)),
            Tok::Ident(n) => {
                if self.t.get(self.i) == Some(&Tok::LParen) {
                    self.i += 1;
                    let mut args = vec![];
                    if self.t.get(self.i) != Some(&Tok::RParen) {
                        loop {
                            args.push(self.or()?);
                            match self.t.get(self.i) {
                                Some(Tok::Comma) => self.i += 1,
                                Some(Tok::RParen) => break,
                                _ => return Err(EvalError::Parse(format!("expected `,` or `)` in call to {n}"))),
                            }
                        }
                    }
                    self.i += 1;
                    Ok(Expr::Call(n, args))
                } else {
                    Ok(Expr::Ident(n))
                }
            }
            Tok::LParen => {
                let e = self.or()?;
                if self.t.get(self.i) != Some(&Tok::RParen) {
                    return Err(EvalError::Parse("missing `)`".into()));
                }
                self.i += 1;
                Ok(e)
            }
            other => Err(EvalError::Parse(format!("unexpected token {other:?}"))),
        }
    }
}

pub fn parse(src: &str) -> Result<Expr, EvalError> {
    let t = lex(src)?;
    if t.is_empty() {
        return Err(EvalError::Parse("empty expression".into()));
    }
    let mut p = P { t, i: 0 };
    let e = p.or()?;
    if p.i != p.t.len() {
        return Err(EvalError::Parse(format!("trailing input in `{src}`")));
    }
    Ok(e)
}

// ---------------------------------------------------------------- evaluator

fn b(x: bool) -> f64 {
    if x { 1.0 } else { 0.0 }
}

fn name_arg(e: &Expr, f: &str) -> Result<String, EvalError> {
    match e {
        Expr::Ident(n) | Expr::Str(n) => Ok(n.clone()),
        _ => Err(EvalError::Arity(format!("{f}() expects a metric name as its first argument"))),
    }
}

impl Expr {
    pub fn eval(&self, ctx: &dyn Context) -> Result<f64, EvalError> {
        match self {
            Expr::Num(v) => Ok(*v),
            Expr::Str(s) => Err(EvalError::Parse(format!("string \"{s}\" used as a value"))),
            Expr::Ident(n) => ctx.var(n).ok_or_else(|| EvalError::Unknown(n.clone())),
            Expr::Neg(e) => Ok(-e.eval(ctx)?),
            Expr::Not(e) => Ok(b(e.eval(ctx)? == 0.0)),
            Expr::Bin(l, op, r) => {
                // short-circuit logic
                match op {
                    Op::And => return Ok(b(l.eval(ctx)? != 0.0 && r.eval(ctx)? != 0.0)),
                    Op::Or => return Ok(b(l.eval(ctx)? != 0.0 || r.eval(ctx)? != 0.0)),
                    _ => {}
                }
                let (a, c) = (l.eval(ctx)?, r.eval(ctx)?);
                Ok(match op {
                    Op::Add => a + c,
                    Op::Sub => a - c,
                    Op::Mul => a * c,
                    Op::Div => if c == 0.0 { f64::NAN } else { a / c },
                    Op::Lt => b(a < c),
                    Op::Le => b(a <= c),
                    Op::Gt => b(a > c),
                    Op::Ge => b(a >= c),
                    Op::Eq => b((a - c).abs() < 1e-9),
                    Op::Ne => b((a - c).abs() >= 1e-9),
                    Op::And | Op::Or => unreachable!(),
                })
            }
            Expr::Call(f, args) => {
                let num = |i: usize| -> Result<f64, EvalError> {
                    args.get(i).ok_or_else(|| EvalError::Arity(format!("{f}() missing argument {}", i + 1)))?.eval(ctx)
                };
                let opt_days = |i: usize, d: u32| -> Result<u32, EvalError> {
                    if args.len() > i { Ok(num(i)?.max(1.0) as u32) } else { Ok(d) }
                };
                let need = |v: Option<f64>, what: String| v.ok_or(EvalError::Unknown(what));
                match f.as_str() {
                    "min" | "max" => {
                        if args.is_empty() {
                            return Err(EvalError::Arity(format!("{f}() needs arguments")));
                        }
                        let vals = args.iter().map(|a| a.eval(ctx)).collect::<Result<Vec<_>, _>>()?;
                        Ok(if f == "min" { vals.into_iter().fold(f64::INFINITY, f64::min) }
                           else { vals.into_iter().fold(f64::NEG_INFINITY, f64::max) })
                    }
                    "abs" => Ok(num(0)?.abs()),
                    "clamp" => Ok(num(0)?.clamp(num(1)?, num(2)?)),
                    "z" => { let n = name_arg(&args[0], f)?; need(ctx.z(&n), format!("z({n})")) }
                    "trend" => { let n = name_arg(&args[0], f)?; let d = opt_days(1, 7)?; need(ctx.trend(&n, d), format!("trend({n})")) }
                    "days_since" => { let n = name_arg(&args[0], f)?; need(ctx.days_since(&n), format!("days_since({n})")) }
                    "mean" => { let n = name_arg(&args[0], f)?; let d = opt_days(1, 28)?; need(ctx.mean(&n, d), format!("mean({n})")) }
                    "prev" => { let n = name_arg(&args[0], f)?; let d = opt_days(1, 1)?; need(ctx.prev(&n, d), format!("prev({n})")) }
                    _ => Err(EvalError::UnknownFunction(f.clone())),
                }
            }
        }
    }

    pub fn truthy(&self, ctx: &dyn Context) -> Result<bool, EvalError> {
        let v = self.eval(ctx)?;
        Ok(v != 0.0 && !v.is_nan())
    }

    /// Plain identifiers referenced (not function names; function arguments that are
    /// metric names are reported as `fn(name)`).
    pub fn identifiers(&self, out: &mut Vec<String>) {
        match self {
            Expr::Ident(n) => if !out.contains(n) { out.push(n.clone()) },
            Expr::Neg(e) | Expr::Not(e) => e.identifiers(out),
            Expr::Bin(l, _, r) => { l.identifiers(out); r.identifiers(out); }
            Expr::Call(f, a) => {
                if matches!(f.as_str(), "z" | "trend" | "days_since" | "mean" | "prev") {
                    if let Some(Expr::Ident(n) | Expr::Str(n)) = a.first() {
                        let k = format!("{f}({n})");
                        if !out.contains(&k) { out.push(k); }
                    }
                    for x in a.iter().skip(1) { x.identifiers(out); }
                } else {
                    for x in a { x.identifiers(out); }
                }
            }
            _ => {}
        }
    }

    /// Metric names referenced anywhere (including inside z()/trend()).
    pub fn metrics(&self) -> Vec<String> {
        let mut ids = vec![];
        self.identifiers(&mut ids);
        ids.into_iter()
            .map(|s| match s.find('(') { Some(i) if s.ends_with(')') => s[i + 1..s.len() - 1].to_string(), _ => s })
            .collect()
    }
}

/// Evaluate every identifier an expression references, for the human-legible trace.
pub fn bindings(e: &Expr, ctx: &dyn Context) -> BTreeMap<String, f64> {
    let mut ids = vec![];
    e.identifiers(&mut ids);
    let mut out = BTreeMap::new();
    for id in ids {
        if let Ok(x) = parse(&id).and_then(|p| p.eval(ctx)) {
            out.insert(id, x);
        }
    }
    out
}

/// Convenience: parse and evaluate in one go.
pub fn eval_str(src: &str, ctx: &dyn Context) -> Result<f64, EvalError> {
    parse(src)?.eval(ctx)
}

pub fn eval_bool(src: &str, ctx: &dyn Context) -> Result<bool, EvalError> {
    parse(src)?.truthy(ctx)
}
