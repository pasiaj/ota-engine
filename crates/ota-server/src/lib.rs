//! # ota-server
//!
//! HTTP API over the store plus run control (§8, §12). A company binary embeds it:
//!
//! ```no_run
//! # use std::sync::Arc;
//! # fn make_world() -> Box<dyn ota_runtime::World> { unimplemented!() }
//! # async fn f() -> anyhow::Result<()> {
//! let cfg = ota_server::ServerConfig::from_env("company", "data");
//! ota_server::serve(Arc::new(make_world), cfg).await
//! # }
//! ```
//!
//! Reads come straight from the files in `data/` (so they never block on a run).
//! `POST /run` and `POST /human-input` require the `X-OTA-Admin` header to equal
//! `OTA_ADMIN_TOKEN`; without a configured token they are disabled.

use anyhow::Result;
use axum::extract::{Path as AxPath, Query, State};
use axum::http::{header, HeaderMap, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Timelike;
use ota_core::*;
use ota_llm::LlmMode;
use ota_runtime::{Engine, EngineOptions, HumanInput, World};
use ota_store::Store;
use ota_tools::ledger::{Ledger, LedgerQuery};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

pub type WorldFactory = Arc<dyn Fn() -> Box<dyn World> + Send + Sync>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Autopilot {
    /// Local hour (0–23) at which one day is advanced.
    pub hour: u32,
    pub timezone: String,
    pub days: u32,
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub bind: SocketAddr,
    /// API prefix, e.g. `/ota/api` (empty = root).
    pub base_path: String,
    pub company_dir: PathBuf,
    pub data_dir: PathBuf,
    /// Optional static viewer directory served at `viewer_path`.
    pub viewer_dir: Option<PathBuf>,
    pub viewer_path: String,
    pub admin_token: Option<String>,
    pub autopilot: Option<Autopilot>,
    pub llm_mode: LlmMode,
    pub scripted_until: Day,
    pub max_days_per_run: u32,
}

impl ServerConfig {
    /// Defaults overridable by env: `OTA_BIND` (0.0.0.0:8080), `OTA_BASE_PATH` (/ota/api),
    /// `OTA_VIEWER_DIR`, `OTA_VIEWER_PATH` (/ota/sim), `OTA_ADMIN_TOKEN`,
    /// `OTA_AUTOPILOT_HOUR` (unset = off), `OTA_TZ` (Europe/Helsinki),
    /// `OTA_LLM_MODE` (auto|live|scripted), `OTA_SCRIPTED_UNTIL`, `OTA_MAX_DAYS_PER_RUN` (31).
    pub fn from_env(company_dir: impl Into<PathBuf>, data_dir: impl Into<PathBuf>) -> Self {
        let env = |k: &str| std::env::var(k).ok().filter(|s| !s.is_empty());
        ServerConfig {
            bind: env("OTA_BIND").and_then(|s| s.parse().ok()).unwrap_or_else(|| "0.0.0.0:8080".parse().unwrap()),
            base_path: env("OTA_BASE_PATH").unwrap_or_else(|| "/ota/api".into()),
            company_dir: company_dir.into(),
            data_dir: data_dir.into(),
            viewer_dir: env("OTA_VIEWER_DIR").map(PathBuf::from),
            viewer_path: env("OTA_VIEWER_PATH").unwrap_or_else(|| "/ota/sim".into()),
            admin_token: env("OTA_ADMIN_TOKEN"),
            autopilot: env("OTA_AUTOPILOT_HOUR").and_then(|h| h.parse().ok()).map(|hour| Autopilot {
                hour, timezone: env("OTA_TZ").unwrap_or_else(|| "Europe/Helsinki".into()), days: 1,
            }),
            llm_mode: match env("OTA_LLM_MODE").as_deref() { Some("live") => LlmMode::Live, Some("scripted") => LlmMode::Scripted, _ => LlmMode::Auto },
            scripted_until: env("OTA_SCRIPTED_UNTIL").and_then(|s| s.parse().ok()).unwrap_or(0),
            max_days_per_run: env("OTA_MAX_DAYS_PER_RUN").and_then(|s| s.parse().ok()).unwrap_or(31),
        }
    }
}

/// Progress of the background run (`GET /run/status`, also `data/run_status.json`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunStatus {
    pub running: bool,
    /// "api" | "autopilot"
    pub trigger: Option<String>,
    pub requested_days: u32,
    pub done_days: u32,
    pub start_day: Option<Day>,
    pub current_day: Option<Day>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub cost_usd: f64,
    pub llm_mode: Option<String>,
    pub errors: Vec<String>,
    pub last_autopilot_date: Option<String>,
}

#[derive(Clone)]
struct AppState {
    cfg: Arc<ServerConfig>,
    factory: WorldFactory,
    store: Store,
    status: Arc<Mutex<RunStatus>>,
}

/// Build the router (useful for tests and for embedding in a larger app).
pub fn router(factory: WorldFactory, cfg: ServerConfig) -> Result<Router> {
    let store = Store::open(&cfg.data_dir)?;
    let mut status: RunStatus = store.read_json("run_status.json").ok().flatten().unwrap_or_default();
    status.running = false;
    let st = AppState { cfg: Arc::new(cfg.clone()), factory, store, status: Arc::new(Mutex::new(status)) };
    let api = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/meta", get(meta))
        .route("/days", get(days))
        .route("/day/:d", get(day))
        .route("/day/:d/module/:m", get(day_module))
        .route("/series", get(series))
        .route("/ledger", get(ledger))
        .route("/decision/:id", get(decision))
        .route("/state", get(state_view))
        .route("/calibration/person/:id", get(person_calibration))
        .route("/run", post(run))
        .route("/run/status", get(run_status))
        .route("/human-input", post(human_input))
        .with_state(st.clone());
    let cors = CorsLayer::new().allow_origin(Any).allow_methods([Method::GET, Method::OPTIONS]).allow_headers(Any);
    let base = cfg.base_path.trim_end_matches('/');
    let mut app = if base.is_empty() { api } else { Router::new().nest(base, api) };
    if let Some(dir) = &cfg.viewer_dir {
        let vp = cfg.viewer_path.trim_end_matches('/');
        let serve = ServeDir::new(dir).append_index_html_on_directories(true);
        app = if vp.is_empty() { app.fallback_service(serve) } else { app.nest_service(vp, serve) };
    }
    if let Some(ap) = cfg.autopilot.clone() {
        tokio::spawn(autopilot(st, ap));
    }
    Ok(app.layer(cors))
}

/// Serve until Ctrl-C.
pub async fn serve(factory: WorldFactory, cfg: ServerConfig) -> Result<()> {
    let bind = cfg.bind;
    let base = cfg.base_path.clone();
    let app = router(factory, cfg)?;
    let listener = tokio::net::TcpListener::bind(bind).await?;
    tracing::info!("OTA server on http://{bind}{base}");
    axum::serve(listener, app).with_graceful_shutdown(async { let _ = tokio::signal::ctrl_c().await; }).await?;
    Ok(())
}

// ---------------------------------------------------------------- helpers

struct ApiError(StatusCode, String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"error": self.1}))).into_response()
    }
}

type ApiResult<T> = std::result::Result<T, ApiError>;

fn not_found(what: impl Into<String>) -> ApiError {
    ApiError(StatusCode::NOT_FOUND, what.into())
}

fn internal(e: impl std::fmt::Display) -> ApiError {
    ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

fn raw_json(bytes: Vec<u8>) -> Response {
    ([(header::CONTENT_TYPE, "application/json")], bytes).into_response()
}

fn check_admin(st: &AppState, h: &HeaderMap) -> ApiResult<()> {
    let Some(tok) = st.cfg.admin_token.as_deref() else {
        return Err(ApiError(StatusCode::FORBIDDEN, "admin actions disabled: OTA_ADMIN_TOKEN is not set".into()));
    };
    match h.get("x-ota-admin").and_then(|v| v.to_str().ok()) {
        Some(t) if constant_eq(t.as_bytes(), tok.as_bytes()) => Ok(()),
        _ => Err(ApiError(StatusCode::UNAUTHORIZED, "missing or wrong X-OTA-Admin token".into())),
    }
}

fn constant_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn day_range(store: &Store) -> Value {
    let d = store.days();
    json!({"first": d.first(), "last": d.last(), "count": d.len()})
}

// ---------------------------------------------------------------- handlers

async fn meta(State(st): State<AppState>) -> ApiResult<Json<Value>> {
    let mut m: Value = st.store.read_json("meta.json").map_err(internal)?.ok_or_else(|| not_found("no meta.json yet: run the simulation first"))?;
    m["days"] = day_range(&st.store);
    let s = st.status.lock().unwrap().clone();
    m["run"] = json!({"running": s.running, "current_day": s.current_day, "llm_mode": s.llm_mode});
    m["live_llm_available"] = json!(std::env::var("OPENAI_API_KEY").map(|k| !k.is_empty()).unwrap_or(false));
    m["admin_enabled"] = json!(st.cfg.admin_token.is_some());
    m["autopilot"] = json!(st.cfg.autopilot);
    Ok(Json(m))
}

async fn days(State(st): State<AppState>) -> Json<Value> {
    let ds = st.store.days();
    Json(json!({"days": ds, "range": day_range(&st.store)}))
}

async fn day(State(st): State<AppState>, AxPath(d): AxPath<Day>) -> ApiResult<Response> {
    st.store.read_day_raw(d).map(raw_json).ok_or_else(|| not_found(format!("day {d} not found")))
}

async fn day_module(State(st): State<AppState>, AxPath((d, m)): AxPath<(Day, String)>) -> ApiResult<Json<Value>> {
    let rec = st.store.read_day(d).map_err(internal)?.ok_or_else(|| not_found(format!("day {d} not found")))?;
    let run = rec.module_runs.iter().find(|r| r.module == m).cloned();
    let messages_in: Vec<&Message> = rec.messages.iter().filter(|x| x.to.contains(&m)).collect();
    let messages_out: Vec<&Message> = rec.messages.iter().filter(|x| x.from == m).collect();
    let decisions: Vec<&Decision> = rec.decisions.iter().filter(|x| x.module == m).collect();
    let updates: Vec<&Decision> = rec.decision_updates.iter().filter(|x| x.module == m).collect();
    let routed: Vec<&Deviation> = rec.deviations.iter().filter(|x| x.routed_to.contains(&m)).collect();
    let actions: Vec<&Action> = rec.actions.iter().filter(|x| x.module == m).collect();
    if run.is_none() && decisions.is_empty() && messages_in.is_empty() && messages_out.is_empty() {
        return Err(not_found(format!("no activity for module {m} on day {d}")));
    }
    Ok(Json(json!({"day": d, "date": rec.date, "module": m, "run": run, "messages_in": messages_in, "messages_out": messages_out,
                   "decisions": decisions, "decision_updates": updates, "deviations": routed, "actions": actions})))
}

#[derive(Deserialize)]
struct SeriesQ {
    metrics: Option<String>,
    from: Option<Day>,
    to: Option<Day>,
}

async fn series(State(st): State<AppState>, Query(q): Query<SeriesQ>) -> Json<Value> {
    let rows = st.store.read_log("kpis");
    let wanted: Option<Vec<String>> = q.metrics.map(|m| m.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect());
    let mut days = vec![];
    let mut dates = vec![];
    let mut series: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let rows: Vec<&Value> = rows.iter().filter(|r| {
        let d = r["day"].as_u64().unwrap_or(0) as Day;
        q.from.map(|f| d >= f).unwrap_or(true) && q.to.map(|t| d <= t).unwrap_or(true)
    }).collect();
    let names: Vec<String> = match &wanted {
        Some(w) => w.clone(),
        None => {
            let mut s: Vec<String> = rows.iter().flat_map(|r| r.as_object().map(|o| o.keys().cloned().collect::<Vec<_>>()).unwrap_or_default())
                .filter(|k| k != "day" && k != "date").collect();
            s.sort();
            s.dedup();
            s
        }
    };
    for r in &rows {
        days.push(r["day"].clone());
        dates.push(r["date"].clone());
        for n in &names {
            series.entry(n.clone()).or_default().push(r.get(n).cloned().unwrap_or(Value::Null));
        }
    }
    Json(json!({"days": days, "dates": dates, "series": series}))
}

fn load_ledger(st: &AppState) -> ApiResult<Ledger> {
    Ok(st.store.load_state::<Ledger>("ledger").map_err(internal)?.unwrap_or_default())
}

async fn ledger(State(st): State<AppState>, Query(q): Query<LedgerQuery>) -> ApiResult<Json<Value>> {
    let l = load_ledger(&st)?;
    let hits = l.search(&q);
    Ok(Json(json!({"total": l.decisions.len(), "count": hits.len(), "decisions": hits})))
}

async fn decision(State(st): State<AppState>, AxPath(id): AxPath<String>) -> ApiResult<Json<Value>> {
    let l = load_ledger(&st)?;
    let d = l.get(&id).cloned().ok_or_else(|| not_found(format!("decision {id} not found")))?;
    let rec = st.store.read_day(d.when).map_err(internal)?;
    let (pm, chs, actions, forecasts) = match &rec {
        Some(r) => (
            d.premortem.as_ref().and_then(|p| r.premortems.iter().find(|x| &x.id == p).cloned()),
            r.challenges.iter().filter(|c| d.challenges.contains(&c.id)).cloned().collect::<Vec<_>>(),
            r.actions.iter().filter(|a| a.decision == d.id).cloned().collect::<Vec<_>>(),
            r.forecasts_made.iter().filter(|f| f.decision.as_deref() == Some(&d.id)).cloned().collect::<Vec<_>>(),
        ),
        None => (None, vec![], vec![], vec![]),
    };
    let assumptions: Value = st.store.load_state::<Value>("assumptions").ok().flatten()
        .and_then(|v| v["items"].as_array().cloned())
        .map(|a| Value::Array(a.into_iter().filter(|x| x["id"].as_str().map(|i| d.assumptions.iter().any(|y| y == i)).unwrap_or(false)).collect()))
        .unwrap_or(Value::Array(vec![]));
    Ok(Json(json!({"decision": d, "premortem": pm, "challenges": chs, "actions": actions, "forecasts": forecasts, "assumptions": assumptions})))
}

async fn state_view(State(st): State<AppState>) -> ApiResult<Response> {
    st.store.read_raw("state/view.json").map(raw_json).ok_or_else(|| not_found("no state yet: run the simulation first"))
}

async fn person_calibration(State(st): State<AppState>, AxPath(id): AxPath<String>, h: HeaderMap) -> ApiResult<Response> {
    // §13.5: individual human calibration is private; only the admin (acting for the person) may read it.
    check_admin(&st, &h)?;
    if id.contains(['/', '.']) {
        return Err(not_found("bad id"));
    }
    st.store.read_raw(&format!("state/private/{id}.json")).map(raw_json).ok_or_else(|| not_found(format!("no profile for {id}")))
}

#[derive(Deserialize)]
struct RunReq {
    days: Option<u32>,
}

async fn run(State(st): State<AppState>, h: HeaderMap, body: Option<Json<RunReq>>) -> ApiResult<Json<Value>> {
    check_admin(&st, &h)?;
    let n = body.and_then(|b| b.days).unwrap_or(1).clamp(1, st.cfg.max_days_per_run);
    start_run(&st, n, "api").map_err(|e| ApiError(StatusCode::CONFLICT, e))?;
    Ok(Json(json!({"started": true, "days": n})))
}

async fn run_status(State(st): State<AppState>) -> Json<RunStatus> {
    Json(st.status.lock().unwrap().clone())
}

async fn human_input(State(st): State<AppState>, h: HeaderMap, Json(mut input): Json<HumanInput>) -> ApiResult<Json<Value>> {
    check_admin(&st, &h)?;
    if input.verdict.is_none() && input.framing.is_none() {
        return Err(ApiError(StatusCode::BAD_REQUEST, "need `verdict` (with `item` or `decision`) or `framing`".into()));
    }
    let now = chrono::Utc::now();
    input.submitted_at = Some(now.to_rfc3339());
    let name = format!("inbox/{}-{}.json", now.format("%Y%m%dT%H%M%S%.6f"), input.person.replace(['/', '.'], "_"));
    st.store.write_json(&name, &input).map_err(internal)?;
    Ok(Json(json!({"queued": true, "applies_on_day": st.store.last_day().unwrap_or(0) + 1, "file": name})))
}

// ---------------------------------------------------------------- background runs

fn persist_status(st: &AppState) {
    let s = st.status.lock().unwrap().clone();
    let _ = st.store.write_json("run_status.json", &s);
}

fn start_run(st: &AppState, n: u32, trigger: &str) -> std::result::Result<(), String> {
    {
        let mut s = st.status.lock().unwrap();
        if s.running {
            return Err(format!("a run is already in progress (day {:?})", s.current_day));
        }
        let last = s.last_autopilot_date.clone();
        *s = RunStatus { running: true, trigger: Some(trigger.into()), requested_days: n, started_at: Some(chrono::Utc::now().to_rfc3339()),
                         last_autopilot_date: last, ..Default::default() };
    }
    persist_status(st);
    let st2 = st.clone();
    std::thread::spawn(move || {
        let res = (|| -> Result<()> {
            let spec = ota_spec::load_company(&st2.cfg.company_dir).map_err(|e| anyhow::anyhow!("{e}"))?;
            let mut opts = EngineOptions::new(&st2.cfg.data_dir);
            opts.llm_mode = st2.cfg.llm_mode;
            opts.scripted_until = st2.cfg.scripted_until;
            let mut e = Engine::new(spec, (st2.factory)(), opts)?;
            {
                let mut s = st2.status.lock().unwrap();
                s.start_day = Some(e.day() + 1);
                s.llm_mode = Some(e.llm.mode_label().into());
            }
            let st3 = st2.clone();
            e.run(n, Some(&mut |r: &DayRecord, done: u32, _t: u32| {
                {
                    let mut s = st3.status.lock().unwrap();
                    s.done_days = done;
                    s.current_day = Some(r.day);
                    s.cost_usd += r.metrics.llm_cost_usd;
                }
                persist_status(&st3);
            }))?;
            Ok(())
        })();
        {
            let mut s = st2.status.lock().unwrap();
            s.running = false;
            s.finished_at = Some(chrono::Utc::now().to_rfc3339());
            if let Err(e) = res {
                s.errors.push(format!("{e:#}"));
            }
        }
        persist_status(&st2);
    });
    Ok(())
}

async fn autopilot(st: AppState, ap: Autopilot) {
    let tz: chrono_tz::Tz = ap.timezone.parse().unwrap_or(chrono_tz::Europe::Helsinki);
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        let now = chrono::Utc::now().with_timezone(&tz);
        let today = now.format("%Y-%m-%d").to_string();
        let due = {
            let s = st.status.lock().unwrap();
            now.hour() == ap.hour && s.last_autopilot_date.as_deref() != Some(&today) && !s.running
        };
        if due {
            st.status.lock().unwrap().last_autopilot_date = Some(today.clone());
            if let Err(e) = start_run(&st, ap.days, "autopilot") {
                tracing::warn!("autopilot: {e}");
            }
        }
    }
}
