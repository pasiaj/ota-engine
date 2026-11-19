//! # ota-llm
//!
//! LLM access for the engine:
//! * tiers from `llm.toml` ([`LlmConfig`]), any OpenAI-compatible chat-completions endpoint,
//! * JSON-schema structured output (`response_format: json_schema`, strict),
//! * token and cost accounting, a daily budget with tier fallback (recorded on the call),
//! * an on-disk response cache keyed by the request hash (replays cost nothing),
//! * retries with exponential backoff,
//! * a [`Scripted`] provider supplied by the company, used for history and whenever no
//!   API key is configured.

pub mod config;
mod openai;

pub use config::*;
pub use openai::OpenAiProvider;

use ota_core::{Day, LlmCallRecord};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Arc;

/// A structured request. `context` carries the same information as `user` in structured
/// form, so a [`Scripted`] responder can answer without parsing prose.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequest {
    /// Tier name in `llm.toml` (`small`, `large`, `env`, `human`, or a custom tier).
    pub tier: String,
    /// What the call is for: `module_think`, `lattice_interpret`, `premortem`,
    /// `adversarial`, `standin`, `brief`, `env_events`, `meta_loop`, `scenario` ...
    pub purpose: String,
    #[serde(default)]
    pub module: Option<String>,
    pub system: String,
    pub user: String,
    pub schema_name: String,
    #[serde(default)]
    pub schema: Option<Value>,
    #[serde(default)]
    pub context: Value,
    pub day: Day,
}

/// Company-supplied deterministic responder. Must return JSON matching `req.schema`.
pub trait Scripted: Send + Sync {
    fn respond(&self, req: &LlmRequest) -> Value;
}

/// Raw completion from a live provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Completion {
    pub content: String,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub model: String,
}

#[derive(Debug)]
pub enum ProviderError {
    /// Worth retrying (429, 5xx, network, malformed JSON).
    Transient(String),
    Fatal(String),
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderError::Transient(m) => write!(f, "transient: {m}"),
            ProviderError::Fatal(m) => write!(f, "fatal: {m}"),
        }
    }
}

pub trait Provider: Send + Sync {
    fn complete(&self, tier: &TierConfig, req: &LlmRequest) -> Result<Completion, ProviderError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmMode {
    /// Live if an API key is present, else scripted.
    Auto,
    Live,
    Scripted,
}

/// Result of a call. `value` is `None` when the budget forced a fall back to rules
/// (see `record.fallback`).
#[derive(Debug, Clone)]
pub struct LlmResult {
    pub value: Option<Value>,
    pub raw: Option<String>,
    pub record: LlmCallRecord,
}

pub struct LlmClient {
    pub config: LlmConfig,
    scripted: Arc<dyn Scripted>,
    live: Option<Box<dyn Provider>>,
    cache_dir: Option<PathBuf>,
    pub day: Day,
    pub spent_today: f64,
    pub spent_total: f64,
    /// Calls since the last `drain_calls` (the runtime stores them in the day record).
    calls: Vec<LlmCallRecord>,
    /// Override the sleep in tests.
    pub backoff_ms: u64,
}

impl LlmClient {
    /// `mode` Auto picks live when `OPENAI_API_KEY` (or a tier's `api_key_env`) is set.
    pub fn new(config: LlmConfig, scripted: Arc<dyn Scripted>, mode: LlmMode, cache_dir: Option<PathBuf>) -> Self {
        let key_present = config.tiers.values().any(|t| std::env::var(t.api_key_env.as_deref().unwrap_or("OPENAI_API_KEY")).map(|k| !k.is_empty()).unwrap_or(false))
            || std::env::var("OPENAI_API_KEY").map(|k| !k.is_empty()).unwrap_or(false);
        let live: Option<Box<dyn Provider>> = match mode {
            LlmMode::Scripted => None,
            LlmMode::Live | LlmMode::Auto if key_present => Some(Box::new(OpenAiProvider::new())),
            _ => None,
        };
        LlmClient { config, scripted, live, cache_dir, day: 0, spent_today: 0.0, spent_total: 0.0, calls: vec![], backoff_ms: 500 }
    }

    /// Use a custom live provider (tests, other APIs).
    pub fn with_provider(mut self, p: Box<dyn Provider>) -> Self {
        self.live = Some(p);
        self
    }

    pub fn is_live(&self) -> bool {
        self.live.is_some()
    }

    pub fn mode_label(&self) -> &'static str {
        if self.is_live() { "live" } else { "scripted" }
    }

    pub fn set_scripted_only(&mut self, scripted_only: bool) {
        if scripted_only {
            self.live = None;
        }
    }

    pub fn begin_day(&mut self, day: Day) {
        if day != self.day {
            self.day = day;
            self.spent_today = 0.0;
        }
    }

    pub fn budget_left(&self) -> f64 {
        (self.config.daily_budget_usd - self.spent_today).max(0.0)
    }

    pub fn drain_calls(&mut self) -> Vec<LlmCallRecord> {
        std::mem::take(&mut self.calls)
    }

    /// The tiers to try, highest first, starting at `tier`.
    fn chain(&self, tier: &str) -> Vec<String> {
        match self.config.fallback.iter().position(|t| t == tier) {
            Some(i) => self.config.fallback[i..].to_vec(),
            None => vec![tier.to_string()],
        }
    }

    fn estimate(&self, tier: &str, req: &LlmRequest) -> f64 {
        let tin = ((req.system.len() + req.user.len()) / 4) as u64 + 50;
        let tout = self.config.tiers.get(tier).and_then(|t| t.max_tokens).unwrap_or(800) as u64;
        self.config.cost(tier, tin, tout)
    }

    pub fn request_hash(model: &str, req: &LlmRequest) -> String {
        let mut h = Sha256::new();
        h.update(model.as_bytes());
        h.update([0]);
        h.update(req.system.as_bytes());
        h.update([0]);
        h.update(req.user.as_bytes());
        h.update([0]);
        h.update(req.schema.as_ref().map(|s| s.to_string()).unwrap_or_default().as_bytes());
        hex::encode(h.finalize())
    }

    fn cache_path(&self, hash: &str) -> Option<PathBuf> {
        if !self.config.cache {
            return None;
        }
        self.cache_dir.as_ref().map(|d| d.join(&hash[..2]).join(format!("{hash}.json")))
    }

    fn scripted_result(&self, req: &LlmRequest, mut record: LlmCallRecord) -> LlmResult {
        let v = self.scripted.respond(req);
        record.model = "scripted".into();
        record.provider = "scripted".into();
        LlmResult { raw: Some(v.to_string()), value: Some(v), record }
    }

    /// Make a call, honouring budget, cache and retries.
    pub fn call(&mut self, req: &LlmRequest) -> LlmResult {
        let base = LlmCallRecord { tier: req.tier.clone(), requested_tier: req.tier.clone(), purpose: req.purpose.clone(), ..Default::default() };
        if self.live.is_none() {
            let r = self.scripted_result(req, base);
            self.calls.push(r.record.clone());
            return r;
        }
        let mut fallback: Vec<String> = vec![];
        for t in self.chain(&req.tier) {
            let Some(tc) = self.config.tiers.get(&t).cloned() else {
                fallback.push(format!("{t} (not configured)"));
                continue;
            };
            let mut rec = LlmCallRecord { tier: t.clone(), model: tc.model.clone(), provider: tc.provider.clone(), ..base.clone() };
            if !fallback.is_empty() {
                rec.fallback = Some(format!("{} → {t}", fallback.join(" → ")));
            }
            let mut treq = req.clone();
            treq.tier = t.clone();
            let hash = Self::request_hash(&tc.model, &treq);
            if let Some(p) = self.cache_path(&hash) {
                if let Some(c) = std::fs::read_to_string(&p).ok().and_then(|s| serde_json::from_str::<Completion>(&s).ok()) {
                    if let Ok(v) = serde_json::from_str::<Value>(&c.content) {
                        rec.cached = true;
                        rec.tokens_in = c.tokens_in;
                        rec.tokens_out = c.tokens_out;
                        self.calls.push(rec.clone());
                        return LlmResult { value: Some(v), raw: Some(c.content), record: rec };
                    }
                }
            }
            let est = self.estimate(&t, &treq);
            if self.spent_today + est > self.config.daily_budget_usd {
                fallback.push(format!("{t} (budget)"));
                continue;
            }
            match self.complete_with_retries(&tc, &treq) {
                Ok(c) => {
                    rec.tokens_in = c.tokens_in;
                    rec.tokens_out = c.tokens_out;
                    rec.cost_usd = self.config.cost(&t, c.tokens_in, c.tokens_out);
                    self.spent_today += rec.cost_usd;
                    self.spent_total += rec.cost_usd;
                    if let Some(p) = self.cache_path(&hash) {
                        let _ = std::fs::create_dir_all(p.parent().unwrap());
                        let _ = std::fs::write(&p, serde_json::to_string(&c).unwrap_or_default());
                    }
                    let v = serde_json::from_str::<Value>(&c.content).unwrap_or(json!({}));
                    self.calls.push(rec.clone());
                    return LlmResult { value: Some(v), raw: Some(c.content), record: rec };
                }
                Err(e) => {
                    // a failed live call degrades to the scripted responder, visibly
                    rec.error = Some(e.to_string());
                    let r = self.scripted_result(&treq, rec);
                    self.calls.push(r.record.clone());
                    return r;
                }
            }
        }
        let rec = LlmCallRecord { tier: "rule".into(), model: String::new(), provider: "none".into(),
                                  fallback: Some(format!("{} → rule", fallback.join(" → "))), ..base };
        self.calls.push(rec.clone());
        LlmResult { value: None, raw: None, record: rec }
    }

    fn complete_with_retries(&self, tc: &TierConfig, req: &LlmRequest) -> Result<Completion, ProviderError> {
        let p = self.live.as_ref().expect("live provider");
        let mut last = ProviderError::Fatal("no attempt".into());
        for attempt in 0..=self.config.max_retries {
            match p.complete(tc, req) {
                Ok(c) => match serde_json::from_str::<Value>(&c.content) {
                    Ok(_) => return Ok(c),
                    Err(e) => last = ProviderError::Transient(format!("response is not JSON: {e}")),
                },
                Err(ProviderError::Fatal(m)) => return Err(ProviderError::Fatal(m)),
                Err(e) => last = e,
            }
            if attempt < self.config.max_retries && self.backoff_ms > 0 {
                std::thread::sleep(std::time::Duration::from_millis(self.backoff_ms << attempt));
            }
        }
        Err(last)
    }
}

/// Make a JSON schema strict-mode compatible: every object gets
/// `additionalProperties: false` and all its properties `required`.
pub fn strict_schema(mut v: Value) -> Value {
    fn walk(v: &mut Value) {
        if let Value::Object(m) = v {
            if m.get("type").and_then(|t| t.as_str()) == Some("object") {
                if let Some(Value::Object(props)) = m.get("properties") {
                    let keys: Vec<Value> = props.keys().map(|k| Value::String(k.clone())).collect();
                    m.insert("required".into(), Value::Array(keys));
                }
                m.insert("additionalProperties".into(), Value::Bool(false));
            }
            for (_, c) in m.iter_mut() {
                walk(c);
            }
        } else if let Value::Array(a) = v {
            for c in a {
                walk(c);
            }
        }
    }
    walk(&mut v);
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Echo;
    impl Scripted for Echo {
        fn respond(&self, req: &LlmRequest) -> Value {
            json!({"scripted": true, "purpose": req.purpose})
        }
    }

    struct Flaky(AtomicUsize);
    impl Provider for Flaky {
        fn complete(&self, tier: &TierConfig, _req: &LlmRequest) -> Result<Completion, ProviderError> {
            let n = self.0.fetch_add(1, Ordering::SeqCst);
            if n == 0 {
                return Err(ProviderError::Transient("429".into()));
            }
            Ok(Completion { content: r#"{"ok":true}"#.into(), tokens_in: 1000, tokens_out: 500, model: tier.model.clone() })
        }
    }

    fn req(tier: &str, user: &str) -> LlmRequest {
        LlmRequest { tier: tier.into(), purpose: "t".into(), module: None, system: "s".into(), user: user.into(),
                     schema_name: "x".into(), schema: None, context: Value::Null, day: 1 }
    }

    #[test]
    fn scripted_when_no_live() {
        let mut c = LlmClient::new(LlmConfig::default(), Arc::new(Echo), LlmMode::Scripted, None);
        let r = c.call(&req("large", "u"));
        assert_eq!(r.value.unwrap()["scripted"], true);
        assert_eq!(r.record.provider, "scripted");
        assert_eq!(c.drain_calls().len(), 1);
    }

    #[test]
    fn retries_cost_cache_and_budget_fallback() {
        let dir = std::env::temp_dir().join(format!("ota-llm-cache-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut cfg = LlmConfig::default();
        cfg.daily_budget_usd = 0.0021;
        let mut c = LlmClient::new(cfg, Arc::new(Echo), LlmMode::Scripted, Some(dir.clone()))
            .with_provider(Box::new(Flaky(AtomicUsize::new(0))));
        c.backoff_ms = 0;
        c.begin_day(1);
        let r = c.call(&req("large", "hello"));
        assert_eq!(r.value.unwrap()["ok"], true);
        assert_eq!(r.record.tier, "large");
        assert!((r.record.cost_usd - (1000.0 * 0.4 + 500.0 * 1.6) / 1e6).abs() < 1e-12);
        // same request again: cache hit, free
        let r2 = c.call(&req("large", "hello"));
        assert!(r2.record.cached);
        assert_eq!(r2.record.cost_usd, 0.0);
        // large estimate no longer fits the remaining budget → falls back to small
        let r3 = c.call(&req("large", "different"));
        assert_eq!(r3.record.tier, "small");
        assert!(r3.record.fallback.as_deref().unwrap().contains("large (budget)"));
        // exhaust: everything falls to rule
        c.spent_today = 1.0;
        let r4 = c.call(&req("large", "third"));
        assert!(r4.value.is_none());
        assert_eq!(r4.record.tier, "rule");
        // a new day resets the budget
        c.begin_day(2);
        assert!(c.budget_left() > 0.0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn strict_schema_marks_all_required() {
        let s = strict_schema(json!({"type":"object","properties":{"a":{"type":"string"},"b":{"type":"object","properties":{"c":{"type":"number"}}}}}));
        assert_eq!(s["required"], json!(["a", "b"]));
        assert_eq!(s["properties"]["b"]["additionalProperties"], false);
    }

    #[test]
    fn parses_llm_toml() {
        let c = LlmConfig::parse("daily_budget_usd = 1.5\n[tiers.small]\nmodel = \"m\"\nprice_in = 0.1\n").unwrap();
        assert_eq!(c.daily_budget_usd, 1.5);
        assert_eq!(c.tiers["small"].provider, "openai");
        assert!(LlmConfig::parse("[tiers.small]\nmodel = \"\"\n").is_err());
    }
}
