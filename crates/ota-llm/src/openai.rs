//! OpenAI-compatible chat-completions provider (blocking reqwest, rustls).

use crate::{strict_schema, Completion, LlmRequest, Provider, ProviderError, TierConfig};
use serde_json::{json, Value};
use std::time::Duration;

pub struct OpenAiProvider {
    http: reqwest::blocking::Client,
}

impl Default for OpenAiProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenAiProvider {
    pub fn new() -> Self {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .expect("http client");
        OpenAiProvider { http }
    }
}

impl Provider for OpenAiProvider {
    fn complete(&self, tier: &TierConfig, req: &LlmRequest) -> Result<Completion, ProviderError> {
        let key_env = tier.api_key_env.as_deref().unwrap_or("OPENAI_API_KEY");
        let key = std::env::var(key_env).map_err(|_| ProviderError::Fatal(format!("{key_env} not set")))?;
        let base = tier.base_url.as_deref().unwrap_or("https://api.openai.com/v1").trim_end_matches('/');
        let mut body = json!({
            "model": tier.model,
            "messages": [
                {"role": "system", "content": req.system},
                {"role": "user", "content": req.user},
            ],
        });
        match &req.schema {
            Some(s) => {
                body["response_format"] = json!({
                    "type": "json_schema",
                    "json_schema": {"name": req.schema_name, "strict": true, "schema": strict_schema(s.clone())}
                });
            }
            None => body["response_format"] = json!({"type": "json_object"}),
        }
        if let Some(m) = tier.max_tokens {
            body["max_tokens"] = json!(m);
        }
        if let Some(t) = tier.temperature {
            body["temperature"] = json!(t);
        }
        let mut rb = self.http.post(format!("{base}/chat/completions")).bearer_auth(key).json(&body);
        if let Some(t) = tier.timeout_s {
            rb = rb.timeout(Duration::from_secs(t));
        }
        let resp = rb.send().map_err(|e| ProviderError::Transient(e.to_string()))?;
        let status = resp.status();
        let text = resp.text().map_err(|e| ProviderError::Transient(e.to_string()))?;
        if status.as_u16() == 429 || status.is_server_error() {
            return Err(ProviderError::Transient(format!("HTTP {status}: {}", truncate(&text, 300))));
        }
        if !status.is_success() {
            return Err(ProviderError::Fatal(format!("HTTP {status}: {}", truncate(&text, 300))));
        }
        let v: Value = serde_json::from_str(&text).map_err(|e| ProviderError::Transient(format!("bad body: {e}")))?;
        let content = v["choices"][0]["message"]["content"].as_str()
            .ok_or_else(|| ProviderError::Transient("no content in response".into()))?
            .to_string();
        Ok(Completion {
            content,
            tokens_in: v["usage"]["prompt_tokens"].as_u64().unwrap_or(0),
            tokens_out: v["usage"]["completion_tokens"].as_u64().unwrap_or(0),
            model: v["model"].as_str().unwrap_or(&tier.model).to_string(),
        })
    }
}

fn truncate(s: &str, n: usize) -> &str {
    match s.char_indices().nth(n) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}
