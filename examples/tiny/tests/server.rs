use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use ota_example_tiny::{company_dir, ToyWorld};
use ota_llm::LlmMode;
use ota_runtime::World;
use serde_json::Value;
use std::sync::Arc;
use tower::ServiceExt;

async fn get(app: &axum::Router, uri: &str) -> (StatusCode, Value) {
    let r = app.clone().oneshot(Request::get(uri).body(Body::empty()).unwrap()).await.unwrap();
    let s = r.status();
    let b = r.into_body().collect().await.unwrap().to_bytes();
    (s, serde_json::from_slice(&b).unwrap_or(Value::Null))
}

async fn post(app: &axum::Router, uri: &str, token: Option<&str>, body: &str) -> (StatusCode, Value) {
    let mut rb = Request::post(uri).header("content-type", "application/json");
    if let Some(t) = token { rb = rb.header("x-ota-admin", t); }
    let r = app.clone().oneshot(rb.body(Body::from(body.to_string())).unwrap()).await.unwrap();
    let s = r.status();
    let b = r.into_body().collect().await.unwrap().to_bytes();
    (s, serde_json::from_slice(&b).unwrap_or(Value::Null))
}

#[tokio::test(flavor = "multi_thread")]
async fn api_end_to_end() {
    let dir = std::env::temp_dir().join(format!("ota-tiny-server-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut cfg = ota_server::ServerConfig::from_env(company_dir(), &dir);
    cfg.base_path = "/ota/api".into();
    cfg.admin_token = Some("secret".into());
    cfg.llm_mode = LlmMode::Scripted;
    cfg.autopilot = None;
    let app = ota_server::router(Arc::new(|| Box::new(ToyWorld::default()) as Box<dyn World>), cfg).unwrap();

    assert_eq!(get(&app, "/ota/api/health").await.0, StatusCode::OK);
    assert_eq!(post(&app, "/ota/api/run", None, r#"{"days": 3}"#).await.0, StatusCode::UNAUTHORIZED);
    let (s, v) = post(&app, "/ota/api/run", Some("secret"), r#"{"days": 12}"#).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    for _ in 0..200 {
        let (_, st) = get(&app, "/ota/api/run/status").await;
        if st["running"] == false && st["done_days"] == 12 { break; }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    let (_, st) = get(&app, "/ota/api/run/status").await;
    assert_eq!(st["done_days"], 12, "{st}");
    assert!(st["errors"].as_array().unwrap().is_empty(), "{st}");

    let (s, meta) = get(&app, "/ota/api/meta").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(meta["days"]["last"], 12);
    assert_eq!(meta["company"]["name"], "Tiny Games Oy");
    assert!(meta["llm"]["tiers"]["small"].get("api_key_env").is_none(), "no secrets in meta");

    let (s, day) = get(&app, "/ota/api/day/7").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(day["day"], 7);
    assert_eq!(get(&app, "/ota/api/day/99").await.0, StatusCode::NOT_FOUND);
    let (s, m) = get(&app, "/ota/api/day/7/module/ua").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(m["run"]["module"], "ua");

    let (_, series) = get(&app, "/ota/api/series?metrics=dau,cpi&from=2&to=5").await;
    assert_eq!(series["days"].as_array().unwrap().len(), 4);
    assert_eq!(series["series"]["dau"].as_array().unwrap().len(), 4);

    let (_, l) = get(&app, "/ota/api/ledger?module=ua").await;
    assert!(l["decisions"].as_array().unwrap().iter().all(|d| d["module"] == "ua"));
    if let Some(id) = l["decisions"][0]["id"].as_str() {
        let (s, d) = get(&app, &format!("/ota/api/decision/{id}")).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(d["decision"]["id"], id);
    }
    let (s, state) = get(&app, "/ota/api/state").await;
    assert_eq!(s, StatusCode::OK);
    assert!(state["autonomy"]["rungs"].is_object());
    assert!(state["calibration"]["overall_ai"].is_object());

    // private calibration needs the admin token
    assert_eq!(get(&app, "/ota/api/calibration/person/ceo").await.0, StatusCode::UNAUTHORIZED);

    // real-human input is queued for the next tick
    let (s, v) = post(&app, "/ota/api/human-input", Some("secret"), r#"{"person":"ceo","framing":"Focus on retention, not installs"}"#).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["applies_on_day"], 13);
    let _ = post(&app, "/ota/api/run", Some("secret"), r#"{"days": 1}"#).await;
    for _ in 0..200 {
        let (_, st) = get(&app, "/ota/api/run/status").await;
        if st["running"] == false && st["current_day"] == 13 { break; }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    let (_, d13) = get(&app, "/ota/api/day/13").await;
    assert!(d13["human_actions"].as_array().unwrap().iter().any(|h| h["source"] == "real_human"), "real human framing recorded");
    let _ = std::fs::remove_dir_all(&dir);
}
