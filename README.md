# ota-engine

A company-agnostic Rust implementation of the ideas in Risto Siilasmaa's
*OTA — Havaitse, Ajattele, Toimi* (Observe, Think, Act). A company is described in
human-readable Markdown (modules, decision tables, people), plugs in a market model
through the `World` trait, and the engine runs it one simulated day at a time: every
module observes, thinks (decision tables first, LLMs where they add value) and acts,
under the book's toolkit — Decision Ledger, Assumption Cycle, Adversarial Engine,
Automatic Pre-Mortem, Reality Lattice, Signal Brief, Scenario Tree, Strategy Coherence,
Living OKRs, Calibration Tracker, Autonomy Ladder, Domain Expert Gate, Double-loop
learning, Sentinel, Meta-loop, Independent audit and the success grid.

* Design: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) (§13 amendments override earlier sections)
* Company file formats: [`docs/SPEC-FORMAT.md`](docs/SPEC-FORMAT.md)
* Data/API contract for viewers: [`docs/DATA.md`](docs/DATA.md)

## Crates

| Crate | What it does |
|---|---|
| `ota-core` | Domain types: phases, modalities, the 6 pathogens, the 13 AI failure modes (from ota.js), Sentinel signal classes, rungs, scenario tiers…; records (`Decision` with the 7 fields + switches, `ModuleRun`, `DayRecord`, …). |
| `ota-spec` | Loads a company directory (Markdown + YAML front-matter + Markdown decision tables), validates it, and refuses modules without an `accountable` human. |
| `ota-decide` | The Decisions API: a small deterministic expression language (`z()`, `trend()`, `days_since()`, `mean()`, `prev()`, `min/max/abs/clamp`) and first-match decision tables with `otherwise` and an `escalate` column. |
| `ota-llm` | LLM tiers from `llm.toml`, OpenAI-compatible chat completions with strict JSON-schema output, cost accounting, daily budget with tier fallback recorded on each call, on-disk response cache, retries with backoff, and the company-supplied `Scripted` responder. rustls only (no OpenSSL). |
| `ota-tools` | The toolkit as stateful services (ledger, assumptions, governance helpers for gate/pre-mortem/adversarial, scenarios, coherence, okrs, calibration, autonomy, lattice, sentinel, anomaly journal/double loop/meta-loop/audit, grid, history aggregates). |
| `ota-runtime` | The daily tick (§6) in order, cadences from the calendar, layered rollout from `toolkit.md`, message bus with hop counts, stand-ins, real-human input, metrics, checkpoints. Defines the `World` trait. |
| `ota-store` | `data/days/DNNNN.json`, `data/state/*.json`, append-only `data/log/*.jsonl`, atomic writes. |
| `ota-server` | axum API under a configurable base path, background runs with progress, admin-token protected `POST /run` and `POST /human-input`, optional autopilot (one day per day at a local hour), optional static viewer. |

All eight crates of ARCHITECTURE §2.1 exist as specified; none were merged. The Signal
Brief and the daily metrics live in `ota-runtime` (they assemble outputs of several tools)
rather than as separate tool modules.

## How a company plugs in

1. **Write the spec** (see `docs/SPEC-FORMAT.md`, example in `examples/tiny/company/`):
   `company.md`, `constitution.md`, `toolkit.md`, `llm.toml`, `modules/**/MODULE.md` with
   `decisions/*.md`, `prompts/think.md`, `assumptions.md`, `kpis.md`, and
   `people/<id>/PERSONA.md` + `judgment.md`.
2. **Implement `World`** (the environment and company state):

   ```rust
   pub trait World: Send {
       fn init(&mut self, _spec: &CompanySpec) -> anyhow::Result<()> { Ok(()) }
       /// Step 1a: today's external events (use ctx.llm(...) with tier "env" when ctx.is_live()).
       fn generate_events(&mut self, day: Day, ctx: &mut EventCtx) -> Vec<EnvEvent>;
       /// Step 1b: advance one day given today's events and yesterday's executed actions.
       fn advance(&mut self, day: Day, events: &[EnvEvent], executed: &[Action]) -> anyhow::Result<WorldDay>;
       /// Step 9: apply an approved action; return a one-line effect summary.
       fn apply_action(&mut self, day: Day, action: &Action) -> anyhow::Result<String>;
       /// Deterministic responder for every LLM purpose (history, offline, failures).
       fn scripted(&self) -> Arc<dyn Scripted>;
       fn save(&self) -> serde_json::Value;
       fn load(&mut self, state: serde_json::Value) -> anyhow::Result<()>;
   }

   pub struct WorldDay {
       pub kpis: BTreeMap<String, f64>,   // telemetry → internal signals, lattice input
       pub signals: Vec<Signal>,          // extra signals (reviews, notices) with provenance
       pub anomalies: Vec<Anomaly>,       // observation register entries (end nodes, modules)
       pub research: Vec<EnvEvent>,       // external research / peer-practice feed (meta-loop)
       pub untraced_human_actions: usize, // human actions outside the ledger (Sentinel: bypass)
       pub notes: Vec<String>,
   }
   ```

   `Scripted::respond(&LlmRequest) -> serde_json::Value` must return JSON matching
   `req.schema` for each `req.purpose` (`module_think`, `lattice_interpret`,
   `premortem`, `adversarial`, `standin`, `brief`, `meta_loop`, `env_events`).
   `req.context` carries the same data as the prompt in structured form.
   Action kinds are whatever the company's decision tables write in `then`
   (`adjust` for `x *= 1.1` style rows).
3. **Run it** — offline history and live days use the same pipeline:

   ```rust
   let spec = ota_spec::load_company("company")?;            // all errors at once, with paths
   let mut opts = ota_runtime::EngineOptions::new("data");
   opts.scripted_until = 365;                                // history days always scripted
   let mut engine = ota_runtime::Engine::new(spec, Box::new(MyWorld::new()), opts)?;  // resumes from data/
   engine.run(365, None)?;
   ```
4. **Serve it**:

   ```rust
   let cfg = ota_server::ServerConfig::from_env("company", "data");
   ota_server::serve(Arc::new(|| Box::new(MyWorld::new()) as Box<dyn World>), cfg).await?;
   ```

   Environment: `OTA_BIND` (0.0.0.0:8080), `OTA_BASE_PATH` (/ota/api), `OTA_ADMIN_TOKEN`
   (enables POST endpoints), `OTA_AUTOPILOT_HOUR` (e.g. 3; unset = off), `OTA_TZ`
   (Europe/Helsinki), `OTA_LLM_MODE` (auto|live|scripted), `OTA_SCRIPTED_UNTIL`,
   `OTA_MAX_DAYS_PER_RUN` (31), `OTA_VIEWER_DIR`/`OTA_VIEWER_PATH` (/ota/sim),
   `OPENAI_API_KEY` (live LLM; without it the scripted responder is used).

## Example

```sh
export PATH=$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH
cargo run -p ota-example-tiny -- run 90 /tmp/tiny-data      # offline, deterministic
OTA_ADMIN_TOKEN=secret cargo run -p ota-example-tiny -- serve /tmp/tiny-data
curl localhost:8080/ota/api/day/12 | jq '.module_runs[0].think'
curl -XPOST -H 'X-OTA-Admin: secret' -d '{"days":3}' -H 'content-type: application/json' localhost:8080/ota/api/run
```

## Viewer

`viewer/` is a static, build-free web viewer (vanilla ES modules + SVG) built strictly against
`docs/DATA.md`. It works for any company on the engine. Views (hash-routed, every view/day/module/
decision linkable, e.g. `#/module/ua/day/120`): **Start here** (onboarding, animated daily tick,
guided tour, what is simulated vs real), **Docs** (module format, decision tables, LLM tiers,
every toolkit component, Sentinel signatures, data model), **Birdseye** (org graph, day scrubber,
KPI sparklines, event/Sentinel markers), **Module focus**, **Decision / Ledger**, **Health**,
**People** and **Run** (admin token, human input).

* API base: `../api/` relative to the page (so `/ota/sim/` → `/ota/api/`); override with
  `?api=http://localhost:8080/ota/api/`.
* Serve locally with the server: `OTA_VIEWER_DIR=$PWD/viewer cargo run -p ota-example-tiny -- serve /tmp/tiny-data`
  and open `http://localhost:8080/ota/sim/`.
* Publish: `tools/publish.sh /path/to/xpost` copies it to `nginx/www/pasiaj.com/ota/sim/`.
* Visual identity (`assets/ota.css`, `assets/identity.js`) is copied from the OTA atlas.

## Checks

```sh
cargo build --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets   # if clippy is installed
```

Tests cover the expression language and decision tables, the spec loader (including
refusal of modules without an accountable human and rollout-order violations), the
LLM client (retries, cost, cache, budget fallback), the store, the lattice and
calibration, a 30-day run of the tiny company with resume from disk, and an end-to-end
API test (admin token, background run, real-human input).

## Validating a company

```sh
cargo run -p ota-spec --bin ota-check -- ../louhi-games    # summary, warnings, all errors
```

## Known gaps

* Thresholds of Sentinel signatures, autonomy promotion/demotion and the lattice are
  engine constants (`ota-tools`), not yet configurable from the company spec.
* The success-grid cell health is a heuristic of enabled-tool coverage plus a few
  behaviour metrics (external share, dissent, ledger completeness, real-human share).
* The truth pipeline (§13.10) is only partially modelled: `metric_owner_of` and KPI
  `domain` are loaded and exposed, but "≥3 independent sources per domain", the monthly
  situational-awareness review and the 6-month shadow-group rotation are not enforced.
* The learning rate uses the simulation day as the experience proxy for the Wright curve.
* The leader "think week" blocks capacity on the first day of each quarter only.
* Scenario branches are proposed by rules (large deviations, anomaly clusters), not by an
  LLM scenario prompt.
* `state/*.json` (ledger, view) is written at checkpoints (every 30 days and at the end of
  each run), so it can lag the newest day file during a long run.
* The live OpenAI path is covered by unit tests with a fake provider only.
