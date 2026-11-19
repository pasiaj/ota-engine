//! `llm.toml`: freely defined LLM tiers (§5).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LlmConfig {
    /// Daily spend cap across all tiers, USD.
    #[serde(default = "default_budget")]
    pub daily_budget_usd: f64,
    /// Fallback order when the budget is exhausted, highest first. Below the last entry
    /// the caller falls back to rules (or defers).
    #[serde(default = "default_fallback")]
    pub fallback: Vec<String>,
    #[serde(default = "yes")]
    pub cache: bool,
    #[serde(default = "default_retries")]
    pub max_retries: u32,
    #[serde(default)]
    pub tiers: BTreeMap<String, TierConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TierConfig {
    /// "openai" (any OpenAI-compatible endpoint) or "scripted".
    #[serde(default = "default_provider")]
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub base_url: Option<String>,
    /// Environment variable holding the key (default OPENAI_API_KEY). Never serialised to the viewer.
    #[serde(default)]
    pub api_key_env: Option<String>,
    /// USD per 1M input tokens.
    #[serde(default)]
    pub price_in: f64,
    /// USD per 1M output tokens.
    #[serde(default)]
    pub price_out: f64,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub timeout_s: Option<u64>,
}

fn default_budget() -> f64 { 2.0 }
fn default_fallback() -> Vec<String> { vec!["large".into(), "small".into()] }
fn yes() -> bool { true }
fn default_retries() -> u32 { 3 }
fn default_provider() -> String { "openai".into() }

impl Default for LlmConfig {
    fn default() -> Self {
        let mut tiers = BTreeMap::new();
        let t = |m: &str, i: f64, o: f64| TierConfig {
            provider: "openai".into(), model: m.into(), base_url: None, api_key_env: None,
            price_in: i, price_out: o, max_tokens: None, temperature: None, timeout_s: None,
        };
        tiers.insert("small".into(), t("gpt-4.1-nano", 0.10, 0.40));
        tiers.insert("large".into(), t("gpt-4.1-mini", 0.40, 1.60));
        tiers.insert("env".into(), t("gpt-4.1-mini", 0.40, 1.60));
        tiers.insert("human".into(), t("gpt-4.1-mini", 0.40, 1.60));
        LlmConfig { daily_budget_usd: 2.0, fallback: default_fallback(), cache: true, max_retries: 3, tiers }
    }
}

impl LlmConfig {
    pub fn parse(src: &str) -> Result<LlmConfig, String> {
        let c: LlmConfig = toml::from_str(src).map_err(|e| e.to_string())?;
        for (name, t) in &c.tiers {
            if t.model.trim().is_empty() {
                return Err(format!("tier `{name}` has an empty model"));
            }
        }
        Ok(c)
    }

    /// Tiers without secrets, for `/api/meta`.
    pub fn public_view(&self) -> serde_json::Value {
        let tiers: BTreeMap<_, _> = self.tiers.iter().map(|(k, t)| (k.clone(), serde_json::json!({
            "provider": t.provider, "model": t.model, "base_url": t.base_url,
            "price_in": t.price_in, "price_out": t.price_out,
        }))).collect();
        serde_json::json!({"daily_budget_usd": self.daily_budget_usd, "fallback": self.fallback, "tiers": tiers})
    }

    pub fn cost(&self, tier: &str, tokens_in: u64, tokens_out: u64) -> f64 {
        self.tiers.get(tier).map(|t| (tokens_in as f64 * t.price_in + tokens_out as f64 * t.price_out) / 1e6).unwrap_or(0.0)
    }
}
