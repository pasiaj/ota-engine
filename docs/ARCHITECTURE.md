# OTA Engine: architecture

This is the plan for two repositories:

| Repo | What it is |
|---|---|
| `ota-engine` | Abstract, company-agnostic Rust implementation of the ideas in Risto Siilasmaa's *OTA — Havaitse, Ajattele, Toimi*. Also ships the generic web viewer. |
| `louhi-games` | A concrete company (a fictional Finnish mobile-game studio) built on the engine. It holds the market simulation, the company's modules (as Markdown), its people stand-ins, the one-year history generator and the live daily simulation. |

References like "ch 13" point at chapters of the book. The book is the specification. Where this plan departs from it, the departure is marked **[deviation]** and justified.

---

## 1. Design principles, taken from the book

1. **The loop is the unit.** Every module, at every level, runs Observe → Think → Act and feeds what it learned back (ch 1, flywheel). The company itself is one loop, and the meta-loop is a loop over the loops (ch 13).
2. **Three muscles fail separately.** Observation, thinking and action each have their own instruments, failure modes and metrics. The engine never fuses them into one "AI agent step" that can't be inspected. Each tick stores what was observed, what was thought and what was done as separate records (ch 2).
3. **AI amplifies; it doesn't correct** (ch 12–13). The toolkit is laid on top of human practices, not instead of them. Each AI tool names its human predecessor (`evolves_from`) and keeps its human checkpoint.
4. **Decision rights are explicit** (decision architecture, ch 11). Every decision has a *decision class*. Each class has a reversibility rating, an impact rating, an owner and an autonomy rung. Nothing is decided "by the system" in general.
5. **Authority is earned and lost per decision class** (Autonomy Ladder, ch 13). It moves on calibration evidence (Brier scores) against thresholds agreed in advance, and every change is logged and reversible.
6. **Every decision is a hypothesis with an exit** (constitution principle 1, decision switches). Each ledger entry stores the 7 fields, its assumptions, the expected outcome, and a confirm signal, a stop signal and a check date. *No decision without an outcome* (outcome sealing, ch 16).
7. **Humans do framing, judgment and accountability** (the three human tasks, ch 14). A named human is accountable at the top of every chain (constitution principle 6). Humans sit only at the edges and on the leadership team. The middle layer is AI, plus **domain expert validators** who gate claims (Domain Expert Gate).
8. **The system is a colleague, not an authority** (constitution principle 7). Every recommendation carries its reasoning in a human-readable form (human-legible reasoning) and its confidence. Overrides are logged with reasons.
9. **Layered rollout** (ch 13). The tools depend on one another in a fixed order: Ledger → Assumption Cycle → Adversarial Engine → observation stack (Lattice, Signal Brief) → monitors (Sentinel, Meta-loop). The engine enforces it: a tool can't be enabled before the tools it depends on.
10. **Independent audit** (ch 13). Sentinel watches the toolkit. A separate audit module, with a separate code path and read-only access, watches Sentinel.

---

## 2. Layers of the engine

```
┌──────────────────────────────────────────────────────────────────────┐
│ ENVIRONMENT (company repo)  market model + LLM event generator        │
└───────────────┬──────────────────────────────────────────▲───────────┘
                │ raw signals / telemetry                    │ actions
┌───────────────▼──────────────────────────────────────────┴───────────┐
│ DATA ESTATE  append-only store: signals, metrics, messages, decisions, │
│              assumptions, forecasts, outcomes  (attributable, stamped) │
├──────────────────────────────────────────────────────────────────────┤
│ OBSERVE   Reality Lattice L1–L5 · Signal Brief · Observation register  │
│ THINK     Module agents (decision tables → LLM tiers) · Assumption     │
│           Cycle · Adversarial Engine · Auto Pre-Mortem · Scenario Tree │
│           · Strategy Coherence · Living OKRs                           │
│ GOVERN    Decision Architecture · Autonomy Ladder · Domain Expert Gate │
│           · Human stand-ins (leadership, validators, end nodes)       │
│ ACT       Action executor · Decision Ledger (7 fields + switches)      │
│ LEARN     Outcome sealing · Calibration Tracker · Double-loop learning │
│ MONITOR   Sentinel (6 pathogens + AI failure modes) · Meta-loop ·      │
│           Independent audit · Success-grid self-assessment             │
└──────────────────────────────────────────────────────────────────────┘
```

### 2.1 Crates (Cargo workspace in `ota-engine`)

| Crate | Responsibility |
|---|---|
| `ota-core` | Domain types (§4), ids, simulated clock, `Phase` (O/T/A), `Modality` (5), `Pathogen` (6), `AiFailureMode` (9), `GridCell` (15), errors. |
| `ota-spec` | Loads module and person specs from directory trees of Markdown files with YAML front-matter. Parses **decision tables** written as Markdown tables (§5). Validates references between them. |
| `ota-llm` | Provider abstraction. Any OpenAI-compatible chat endpoint can be configured as a *tier* in `llm.toml`. Supports JSON-schema structured output, response cache (content hash), cost and token accounting, a daily budget, and a `Scripted` provider for offline or historical runs. |
| `ota-decide` | The **Decisions API** (§5): evaluates decision tables, scoring functions and thresholds deterministically. It escalates to an LLM tier only when no rule fires, a rule says `escalate`, or novelty is above the threshold. |
| `ota-tools` | The book's toolkit as stateful services: `ledger`, `assumptions`, `adversarial`, `premortem`, `scenarios`, `coherence`, `okrs`, `calibration`, `autonomy`, `gate`, `lattice`, `brief`, `double_loop`, `sentinel`, `meta_loop`, `audit`, `grid`. |
| `ota-runtime` | The daily tick scheduler (§6), message bus, cadence (daily, weekly, monthly, quarterly), the layered-rollout dependency check, and the `World` trait the company implements. |
| `ota-store` | Day-partitioned JSON persistence (`data/days/D0001.json`, ...) plus `data/state/*.json` snapshots and append-only `*.jsonl` logs. Readable without the program. |
| `ota-server` | An axum HTTP server exposing the store and a run control (§8). It serves the static `viewer/`. |
| `viewer/` | Static vanilla-JS viewer (no build step). It is generic over any company built on the engine. |

The company repo depends on these crates by path (`../ota-engine/crates/...`).

---

## 3. Organisation model (book → engine)

The book's prescription for an AI-first organisation, as applied in the founding narrative: a small leadership team, no middle management, AI-first communication, and humans only at the end nodes, with domain expert validators allowed in the middle layer.

| Node kind | Who | In the engine |
|---|---|---|
| `leader` | 2–4 humans (stand-ins) | Frames goals, approves major scenario branches, decides irreversible classes, owns accountability for each domain. Reads the Signal Brief. |
| `ai_module` | Functional AI organisations (UA, Live Ops, ...) | The working middle. Each is an OTA loop with its own decision classes. |
| `toolkit` | OTA tools (Lattice, Ledger, Sentinel, ...) | Shared infrastructure modules, also visible as nodes. |
| `validator` | Human domain experts (stand-ins) | The Domain Expert Gate: they review yellow/red claims. They never manage. |
| `end_node` | Humans doing work AI shouldn't or can't do alone | Taste, play-feel, community voice, legal signature. They receive tasks and return judgments. |
| `auditor` | Independent audit | Reads only. A separate path from Sentinel. |

**Communication is AI-first.** Every message between nodes goes through the bus as a structured `Message` with an intent (`inform`, `request`, `propose`, `challenge`, `escalate`, `approve`, `reject`, `task`, `report`). A human stand-in never talks to another human through a management hop: messages route to them directly from the AI layer, and the hop count is a tracked metric ("hops to the decider").

---

## 4. Core data model (`ota-core`)

All records carry `id`, `day` (simulation day, 1-based), `ts` (minute within the day), `by` (node id), and `module`. All serialize to JSON with serde.

```rust
enum Phase { Observe, Think, Act }
enum Modality { Direction, Structure, Processes, Capability, Culture }
enum Pathogen { Hubris, Complacency, NormalisationOfDeviance, Groupthink, MisleadingIncentives, Inertia }
enum AiFailureMode { GlassCockpit, DecisionLaundering, DiffusionOfAccountability, AiEchoChamber,
                     AlertFatigue, SyntheticCertainty, Homogenisation, Bypass, Abdication }
enum Reversibility { Reversible, CostlyToReverse, Irreversible }
enum Impact { Local, CrossFunction, CompanyWide }          // who must be informed (ch 4)
enum Rung { Inform=0, Recommend=1, ActWithApproval=2, ActThenReview=3, Autonomous=4 }
enum GateColor { Green, Yellow, Red }
enum Tier { Rule, Small, Large, Human }                    // the decision engine tier used

struct Signal      { id, day, source, kind, metric: Option<String>, value: Option<f64>, text, provenance: Provenance /* internal|external */, tags }
struct Deviation   { id, day, metric, value, baseline, sigma, z, level: LatticeLevel, interpretation: Option<String>, routed_to: Vec<NodeId>, suppressed_by_budget: bool }
struct Message     { id, day, ts, from, to: Vec<NodeId>, intent, subject, body, refs: Vec<Id>, hops: u8 }
struct Forecast    { id, day, by, question, p: f64, resolves_on: Day, outcome: Option<bool>, brier: Option<f64>, topic, horizon_days }
struct Assumption  { id, text, owner, confidence: f64, load_bearing: bool, created: Day, last_confirmed: Day,
                     status /* active|confirmed|refuted|stale|archived */, indicators: Vec<IndicatorRule>, depends: Vec<DecisionId>, history: Vec<ConfidencePoint> }
struct Challenge   { id, day, target /* assumption or decision */, argument, sources: Vec<String>, strength: f64, response: Option<String>, overridden_by: Option<NodeId> }
struct PreMortem   { id, decision, failure_modes: Vec<{text, p, severity}>, load_bearing: Vec<AssumptionId>, skipped: bool, skip_reason }
struct DecisionSwitch { confirm_if: String, stop_if: String, check_by: Day }       // ch 13
struct Decision {                                          // Decision Ledger entry, 7 fields + extensions
  id, class: DecisionClassId, module,
  what: String, when: Day, who: NodeId /* decider */, information: Vec<Id>, assumptions: Vec<AssumptionId>,
  expected: String, switches: DecisionSwitch,              // ← the seven fields
  reversibility, impact, stakes_eur: f64, novelty: f64,
  confidence: f64, forecasts: Vec<ForecastId>, tier: Tier, rung_at_time: Rung,
  reasoning: String /* human-legible */, alternatives: Vec<String>,
  premortem: Option<PreMortemId>, challenges: Vec<ChallengeId>, gate: Option<GateReview>,
  approvals: Vec<Approval>, overrides: Vec<Override>, accountable: NodeId,
  status /* proposed|approved|rejected|executing|sealed|reverted */,
  outcome: Option<Outcome> /* sealed: actual vs expected, deviation text, success: Option<bool> */
}
struct Action      { id, decision, kind, params: serde_json::Value, executed_day, effect_summary }
struct ScenarioNode{ id, parent: Vec<Id> /* DAG */, title, p: f64, leading_indicators: Vec<IndicatorRule>, assumptions, tier_approval /* 1|2|3 */, approved_by }
struct ClaimNode   { id, kind /* mission|strategy|okr|budget|incentive|policy */, text, links }   // Strategy Coherence
struct OkrState    { id, objective, key_results: Vec<{metric, target, current}>, assumptions, setter: NodeId, flagged: Option<String> }
struct AutonomyChange { day, class, from: Rung, to: Rung, evidence: {brier, n, threshold}, approved_by }
struct SentinelFinding { id, day, kind: Pathogen | AiFailureMode | Drift, signature: String, evidence: Vec<Id>, severity, status }
struct GridScore   { quarter, cells: [[f64;3];5] /* modality × phase */, notes }
struct ModuleRun   { day, module, inputs: RunInputs, think: ThinkTrace, outputs: RunOutputs, cost_usd, tokens, duration_ms }
struct ThinkTrace  { tier: Tier, rules_fired: Vec<RuleHit>, prompt_ref: Option<String>, llm_model: Option<String>,
                     llm_request: Option<String>, llm_response: Option<String>, rationale: String }
```

`ModuleRun` is the main record the viewer uses: *for this module, on this day, these were the inputs, this is how it thought, these were the outputs.*

---

## 5. Decision making: decision tables first, LLMs where they add value

Most decisions in an operating company are routine and reversible. The book's Path 1 (ch 11) says these should default to AI autonomy with after-the-fact review. They don't need an LLM, but a transparent, auditable rule. LLMs are spent where judgment, synthesis or language are needed.

**Decision pipeline for a module on a given day:**

1. **Collect the inbox.** Routed deviations, messages, due decision switches, assumption flags, OKR flags and tasks.
2. **Decisions API (tier `Rule`).** For each decision class the module owns, evaluate its decision table. A table row matches on metric conditions and produces a decision proposal with a deterministic confidence. Rows can also produce `escalate: small|large|human`.
3. **LLM tier `Small`** (default `gpt-4.1-nano` or `gpt-4o-mini`) is used for:
   - interpreting flags (Lattice level 4)
   - drafting messages and briefs
   - classifying environment events into signals
   - deciding when a table says `escalate: small`, or when novelty is between 0.3 and 0.6
4. **LLM tier `Large`** (default `gpt-4.1-mini`) is used for:
   - decisions with high stakes or high novelty
   - pre-mortems
   - adversarial challenges for material assumptions
   - scenario-branch proposals
   - meta-loop proposals
5. **Tier `Human`.** A stand-in persona (§7) is invoked with the decision, its challenge document and the gate result. It approves, rejects or amends, and gives a reason. The persona's LLM tier is configured per person.
6. **Batching.** At most one LLM call per module per day for the module's own thinking. All escalated items go into that one structured request (JSON schema returns an array of decisions). Interpretation of lattice flags is batched into one call per day. Responses are cached by request hash, so replays cost nothing.
7. **Budget.** `llm.toml` sets a daily USD cap. When the cap is reached, the engine falls back down the tiers: Large→Small→Rule, or "defer to tomorrow". The fallback is recorded in the trace. That way cost pressure is visible instead of hidden.

**`llm.toml` (freely defined LLMs):**

```toml
daily_budget_usd = 2.00
[tiers.small]  provider = "openai"  model = "gpt-4.1-nano"  base_url = "https://api.openai.com/v1"  price_in = 0.10  price_out = 0.40
[tiers.large]  provider = "openai"  model = "gpt-4.1-mini"  price_in = 0.40  price_out = 1.60
[tiers.env]    provider = "openai"  model = "gpt-4.1-mini"   # environment generator
[tiers.human]  provider = "openai"  model = "gpt-4.1-mini"
# any OpenAI-compatible endpoint (local llama.cpp, OpenRouter, Azure) can be a tier
```

If no `OPENAI_API_KEY` is set, live runs use the `Scripted` provider, which uses the same templates as the history generator, and say so in the viewer.

**Decision-table format** (Markdown, parsed by `ota-spec`):

```markdown
## Decision class: ua.bid_adjust
- reversibility: reversible
- impact: local
- owner: ua-kc2            # module
- accountable: cmo         # named human
- default_rung: act_then_review
- stakes_eur: 500
- forecast: "D7 ROAS of the adjusted campaign ≥ target in 7 days"

| when                                   | then                    | confidence | escalate |
|----------------------------------------|-------------------------|-----------:|----------|
| roas_d7 > target * 1.15 && spend_util > 0.9 | bid *= 1.10        | 0.72       |          |
| roas_d7 < target * 0.80                | bid *= 0.85             | 0.70       |          |
| cpi_z > 3                              | hold                    | 0.40       | small    |
| otherwise                              | hold                    | 0.60       |          |
```

The expression language is small and deterministic: numbers, metric identifiers, `+ - * /`, comparisons, `&& || !`, and the functions `z()`, `trend()` and `days_since()`. It is evaluated in `ota-decide`.

---

## 6. The daily tick (`ota-runtime`)

One simulated day runs these steps in order. Each step is persisted.

| # | Step | Book concept |
|---|---|---|
| 1 | **Environment** advances the market model one day and the **event generator** proposes 0–6 external events (LLM tier `env` live; scripted in history). Events change market parameters with a decay. | the outside world, weak signals |
| 2 | **Data estate ingest**: telemetry and events become `Signal`s with provenance. | data estate, L1 instruments |
| 3 | **Reality Lattice**: L1 coverage check, L2 baselines (28-day mean/sd, weekday seasonality), L3 detection (z-score, trend breaks), L4 interpretation (batched small-LLM call for the top flags), L5 alert routing with an **alert budget** per node per day. The overflow is recorded as `suppressed_by_budget`, so alert fatigue is visible. | Reality Lattice 6 levels, alert fatigue |
| 4 | **Assumption Cycle**: each active assumption's indicators are evaluated. Confirm raises confidence and refute flags it. With no signal the assumption ages, and after 60 days it is `stale`. Effects propagate to scenario probabilities and dependent decisions, and Living OKRs are flagged to the *setter*. | Assumption Cycle, Living OKRs |
| 5 | **Decision switches due**: ledger decisions whose `check_by` is today are evaluated, and the outcome is **sealed**. Forecasts that resolve today are scored. | decision switches, outcome sealing, Calibration Tracker |
| 6 | **Module think** (§5) for every AI module whose cadence fires today, in dependency order (observation modules before action modules). | OTA loop per module |
| 7 | **Governance** for each proposed decision: (a) **Decision Architecture** looks up the class (reversibility, impact, rung); (b) if stakes × novelty is above the threshold, an **Automatic Pre-Mortem** runs, and skipping it is recorded; (c) the **Adversarial Engine** attaches the strongest challenge for each material assumption; (d) the **Domain Expert Gate** colours the decision: yellow notifies the validator, red blocks until the validator responds; (e) the **Autonomy rung** decides whether it executes, waits for approval or only recommends; (f) the decision is written to the **Ledger** with its 7 fields and switches; (g) **impact** determines who is informed. | decision architecture, pre-mortem, red team, gate, ladder, ledger, impact classification |
| 8 | **Human stand-ins** process their queues (approvals, gate reviews, tasks, framing requests). The day records each one's load. | three human tasks |
| 9 | **Act**: approved actions are executed against the company state (budgets, live-ops events, releases, prototype go/kill). Effects reach the environment the next day. | Act |
| 10 | **Weekly (Mon)**: Calibration profiles; **Autonomy Ladder** promotion/demotion proposals, which the accountable leader approves; **Double-loop learning** over the week's AARs and sealed outcomes; **Strategy Coherence** check; scenario-tree review of tier-2 branches. | |
| 11 | **Daily**: **Sentinel** scans traces for pathogen signatures and AI failure modes; the **Signal Brief** for leadership is drafted (small LLM) and approved by a leader. | Sentinel, Signal Brief |
| 12 | **Monthly (day 1)**: **Meta-loop** proposals for toolkit changes; **Independent audit** of Sentinel and Lattice integrity; leadership scenario meeting (tier-1 branch approvals). | Meta-loop, audit |
| 13 | **Quarterly**: **Success-grid self-assessment** (5 modalities × 3 phases), compared with the OTA 200 failure and success profiles; a pathogen scan report. | success grid, OTA 200 |

### 6.1 Sentinel signatures (ch 13, implemented as computable rules over the store)

| Pathogen / failure mode | Signature over the trailing window |
|---|---|
| Hubris | Mean stated confidence rising while the Brier score is flat or worse |
| Complacency | Lattice deviations ≥2σ with no decision within N days; switch checks overdue; AAR completion falling |
| Normalisation of deviance | Accepted exceptions or overrides of the same kind rising week over week |
| Groupthink | Leaders override adversarial challenges at a rate above X%; pre-mortem failure modes with low diversity (embedding/lexical similarity) |
| Misleading incentives | Ledger decisions' spend mix diverging from the stated strategy (coherence gap rising) |
| Inertia | Switch stop-conditions crossed without a stop decision; the stale-assumption count rising (assumption graveyard) |
| Alert fatigue | Suppressed or ignored alerts above budget; acknowledgment latency rising |
| Decision laundering | High-stakes decisions approved with the gate skipped, or LLM rationale copied verbatim as the human reason |
| Diffusion of accountability | A decision without an `accountable` human, or with ≥3 approvers and none named owner |
| Glass cockpit | Human approval time under a threshold for high-stakes items (rubber-stamping) |
| Synthetic certainty | Single-source high-confidence decisions without triangulation |
| AI echo chamber, homogenisation | The share of external-provenance signals in inputs falling; module outputs converging |
| Bypass | Human decisions taken outside the ledger (end-node actions not traced to a decision) |

---

## 7. Human stand-ins

Each person is a directory `people/<id>/` containing:

- `PERSONA.md`, whose front-matter gives the role kind (`leader`, `validator`, `end_node` or `auditor`), the decision classes they're accountable for, their expertise domains, the LLM tier, their weekly capacity in minutes, and a bias profile:
  - `overconfidence: 0.2`
  - `challenge_acceptance: 0.7`
  - `review_diligence: 0.8`
- `judgment.md`, describing how they judge: values, red lines and the framing questions they ask.
- Optionally `memory.md`, which the engine appends to with notable past decisions.

Stand-ins are called with a strict JSON schema: `{decision: approve|reject|amend|defer, reason, amendments, minutes_spent, confidence}`. The bias profile is passed to the model and also used by the scripted provider. That way the pathogens can genuinely emerge, and Sentinel has something real to find.

---

## 8. Viewer and server

`ota-server` exposes:

| Endpoint | Returns |
|---|---|
| `GET /api/meta` | company, modules, people, decision classes, day range, LLM config (without keys) |
| `GET /api/day/:d` | the full day record: environment, signals, deviations, module runs, messages, decisions, human actions, Sentinel, KPIs |
| `GET /api/day/:d/module/:m` | one `ModuleRun` plus its messages in and out |
| `GET /api/series?metrics=a,b&from&to` | KPI time series |
| `GET /api/ledger?q=&class=&module=&status=` | ledger search |
| `GET /api/state` | current assumptions, scenario tree, autonomy matrix, calibration, grid, Sentinel findings |
| `POST /api/run {days:N}` | advances the live simulation N days in the background |
| `GET /api/run/status` | progress, cost so far, errors |

The viewer's views:

1. **Birdseye (the whole)**: the organisation graph, with leaders at the top, validators as gates on the edges, AI modules in the middle by OTA phase, and end nodes at the rim. For the selected day it shows:
   - edge thickness = message volume, with animated pulses
   - node ring = health (deviations and Sentinel findings)
   - a badge = decisions made at each tier (Rule, Small, Large, Human)
   - a day scrubber with KPI sparklines and event markers
2. **Module focus**: for module *m* on day *d*:
   - inputs (signals, routed deviations, inbox messages, due switches)
   - the think trace (rules fired, which tier, prompt and response, rationale)
   - outputs (decisions, actions, messages, forecasts)
   - a 90-day strip of this module's activity, autonomy rungs per decision class, and calibration
3. **Decision**: the full ledger entry, with its 7 fields, pre-mortem, challenges, gate, approvals, actions, sealed outcome and Brier score, plus links to the assumptions it rests on.
4. **Health**: the Sentinel feed, the pathogen trend, the success grid by quarter against the OTA 200 profile, the autonomy matrix (class × rung over time), calibration curves, the assumption graveyard, the scenario tree with its probabilities, and coherence gaps.
5. **People**: each human's queue, the minutes they spent, their approvals and overrides, and how small the human footprint is.
6. **Run**: advance N days, live progress, cost, and the model used per tier.

---

## 9. The company (`louhi-games`)

**Louhi Games Oy** is a fictional Espoo studio with 14 humans and about 30 AI modules. The simulation's day 1 is 2025-11-20, and its day 365 is 2026-11-19, which is "today".

| Game | Profile |
|---|---|
| **Kallio Climb** (KC1) | A physics hill-driving game, 11 years old. About 99% organic installs (ASO, word of mouth, store features). Ads-heavy monetisation (interstitial and rewarded) plus IAP. Slowly declining DAU, very high brand recognition. |
| **Kallio Climb 2** (KC2) | Multiplayer physics racing with live events, seasons (a battle pass) and an IAP economy. About 80% organic and about 20% paid UA on Meta, Google, AppLovin, Unity and TikTok, with D7 ROAS targets. The main revenue engine. |
| **Project Ember** (EMB) | In development. A hybrid-casual vehicle-merge strategy game designed for **UA profitability at scale**: low CPI creatives, a fast monetisation curve and a D180 LTV/CPI above 1.3. Goes through prototype → CPI test → soft launch (in the simulated year) → scale decision (live). |
| Prototyping funnel | A rapid AI-driven prototype factory: concept → 3-day prototype → CPI market test → kill or continue. Dozens of concepts a year. |

**AI modules**, grouped by organisation. Each is a directory under `modules/`.

| Organisation | Modules |
|---|---|
| Intelligence | `market-intel` (competitors, charts, platform news), `analytics` (KPI models, cohort LTV, experiment readouts), `player-voice` (reviews, support, social) |
| Growth | `ua-kc2`, `ua-ember`, `creative-factory` (ad creative generation and testing), `aso` (store pages, keywords, featuring pitches), `brand-community` |
| Games | `liveops-kc1`, `liveops-kc2`, `economy` (pricing, offers, IAP balance), `ads-monetisation` (mediation, waterfalls, ad frequency), `game-dev-kc2`, `game-dev-ember`, `qa-release`, `prototyping`, `player-support` |
| Business | `portfolio-strategy`, `finance` (cash, budgets, forecasting), `platform-compliance` (ATT, privacy, store policy, age rating) |
| OTA toolkit | `reality-lattice`, `signal-brief`, `decision-ledger`, `assumption-cycle`, `adversarial-engine`, `premortem`, `scenario-tree`, `strategy-coherence`, `living-okrs`, `calibration-tracker`, `autonomy-ladder`, `domain-expert-gate`, `double-loop`, `sentinel`, `meta-loop`, `independent-audit` |

**Humans (stand-ins).** The company has 14 people in total.

| Group | People |
|---|---|
| Leadership (3) | CEO (framing, portfolio, final accountability), Chief Games Officer (creative direction, greenlights), CFO/COO (capital, risk, compliance accountability) |
| Validators (5) | UA and monetisation science, game economy design, data science and methodology, platform and legal, physics/game-feel |
| End nodes (6) | art director, game-feel playtester (KC lineage), community lead (human voice), prototype designer, audio designer, a lawyer for signatures (part-time) |

**Speed.** AI work is fast:
- 60–200 ad creatives a day
- prototypes in 3 days
- CPI tests in 4 days
- daily live-ops tuning
- weekly KC2 content drops

The bottleneck is deliberately human judgment: framing, the validators and the playtesters. The viewer shows that bottleneck.

**Market model** (deterministic seeded Rust, in the company repo). It models:
- organic installs per game, from base × seasonality × store-feature boost × ASO rank × brand-search halo from UA (UA spend lifts organics by about 0.2–0.5× paid installs)
- paid installs from auction curves per network (CPI vs bid, with diminishing returns and audience saturation)
- retention cohorts (D1/D7/D30 curves per game, shifted by updates and quality)
- monetisation (ARPDAU from IAP + ads; ad eCPM by geography and network, seasonal Q4 peak)
- competitors (launches, copycats)
- platform shocks (store policy, privacy changes, SDK deprecations)
- crash rates from releases

LLM events change these parameters with half-life decays.

**History (days 1–365).** It is generated by `louhi history`, which runs the *same* tick pipeline with the `Scripted` provider and an authored **storyline** (`history/storyline.md`, about 40 dated beats). Example beats:
- a KC2 season launch
- an iOS privacy-tracking change that raises CPIs
- a competitor clone
- the first Ember prototype killed
- the second passing its CPI test
- the soft launch
- a normalisation-of-deviance episode in KC2 crash-rate exceptions that Sentinel catches
- an autonomy demotion after a miscalibrated bid run
- a groupthink near-miss on Ember's economy that the adversarial engine flags and leadership first overrides
- an alert-budget retune

The output is fully deterministic (seeded).

**Live (day 366 onward).** `louhi run --days N` (or the viewer's Run button) uses OpenAI models:
- the environment generator (tier `env`) writes the day's events from the market state, recent history and storyline-style constraints, as structured JSON
- modules escalate to Small or Large as their tables dictate
- stand-ins use their tiers

---

## 10. Module directory format (human-readable)

```
modules/growth/ua-kc2/
  MODULE.md              # front-matter: id, name, org, phase(s), cadence, inputs, outputs, owner, accountable,
                         #   evolves_from (human predecessor), depends_on, alert_budget
                         # body: mission, what good looks like, failure modes it watches for
  decisions/
    bid_adjust.md        # decision class + decision table (§5)
    budget_shift.md
    network_test.md
  prompts/
    think.md             # system prompt for the LLM tier (templated with {{inbox}}, {{kpis}}, {{assumptions}})
  assumptions.md         # seed assumptions with indicator rules
  kpis.md                # metrics this module owns and their targets
  playbook.md            # optional: narrative operating guide, readable by humans
```

The engine refuses to load a module whose `MODULE.md` does not name an `accountable` human. That is constitution principle 6.

---

## 11. Validation matrix: book concept → implementation

| Book concept (ch) | Where implemented | How we know it works |
|---|---|---|
| OTA loop, three muscles (1–2) | `ModuleRun` separates the observe inputs, the think trace and the act outputs; runtime steps 3/6/9 | The viewer shows the three columns per module |
| Flywheel, compounding loop (1) | Outcome sealing → calibration → autonomy promotion → faster decisions | Decision latency and the autonomy rung trend over the year |
| Five modalities × three phases, the success grid (6–12) | `grid` tool, quarterly self-assessment; each module and decision class is tagged with a modality | Grid view vs the OTA 200 profile |
| Six pathogens (10) | Sentinel signatures in §6.1 | Storyline episodes are detected in history |
| Observation stack L1–L4, instrument audit (2, 7) | Lattice levels, coverage check, monthly instrument audit by the audit module | Coverage gaps are listed |
| Weak signals, external provenance, input diversity (2, 13) | `Provenance` on signals; echo-chamber signature | External share chart |
| Thinking pipeline TP1–TP6 (3) | Large-tier decision prompt schema requires facts vs inferences, ≥2 hypotheses, load-bearing assumptions, a reference class, second-order effects and falsification conditions | Visible in the think trace |
| AI challenger (3), red team → Adversarial Engine | `adversarial` tool | Challenge docs on decisions |
| Pre-mortem → Automatic Pre-Mortem; skipping recorded | `premortem` tool, governance step 7b | Skip count in Health |
| Decision record → Decision Ledger, 7 fields, decision switches, outcome sealing (4, 13, 16) | `ledger` tool | The Decision view |
| Reversibility, impact classification (4) | Decision-class metadata, governance 7a/7g | Class table |
| Decision architecture, Path 1 / Path 2 (11) | `DecisionClass.default_rung` + reversibility rules | Autonomy matrix |
| Autonomy Ladder, calibrated trust (13) | Weekly promotion/demotion on Brier thresholds, approved by the accountable leader; reversible | Autonomy history |
| Domain Expert Gate green/yellow/red (13) | `gate` tool + validator stand-ins | Gate stats |
| Assumption register → Assumption Cycle; graveyard (13) | `assumptions` tool | Assumption view |
| Continuous scenario mapping → Living Scenario Tree, three-tier governance (13) | `scenarios` | Scenario view |
| OKRs → Living OKRs (flag to the setter) (13) | `okrs` | OKR flags in leaders' inboxes |
| Strategy Coherence Engine, three coherences (13) | `coherence` | Coherence gaps |
| Calibration Tracker, Brier, profiles (13) | `calibration` | Calibration curves |
| AAR → Double-loop learning (5, 13) | `double_loop` weekly | Recurring-pattern findings |
| Signal Brief (13) | `brief` daily, approved by a human | The brief per day |
| Sentinel (13), Meta-loop (13), Independent audit (13) | Monitor layer | Health view |
| Layered rollout (13) | Runtime dependency check | Startup log |
| Data estate (13) | `ota-store`, attributable records | All views |
| Three human tasks: framing, judgment, accountability (14) | Leader stand-ins frame OKRs and scenarios; validators judge; `accountable` is required | People view |
| AI failure modes (14) | Sentinel signatures + design counters (triangulation, legible reasoning, alert budget) | Health view |
| Constitution, 7 principles (17) | `constitution.md` in the company repo; each principle mapped to an enforcement point | Coherence engine checks against it |
| Paranoid optimism (15) | Every decision pairs commit (confidence) with monitor (switches) | Ledger completeness metric |
| Velocity vs speed, decision latency, loop speed, learning rate (metrics) | Computed daily | Birdseye KPIs |
| Truth pipeline, bad-news rule, psychological safety (11) | Bad news (refuted assumptions, negative outcomes) routes to the setter or leader with no suppression; Sentinel tracks the "bad news latency" | Metric in Health |

**[deviation]** The book's tools act on human organisations. Here most of the organisation is AI, so "culture" signals (psychological safety, dissent) are measured on module and stand-in behaviour: the rate of challenges, the override rate, and the diversity of dissent. The deviation is explicit in the viewer.

**[deviation]** The history isn't generated by LLMs, as the brief asks. It comes from templates plus an authored storyline, and goes through the identical pipeline.

---

## 12. Deployment: pasiaj.com/ota

| Path | Served by | Source |
|---|---|---|
| `pasiaj.com/ota/` | xpost `static` nginx (`nginx/www/pasiaj.com/ota/`) | `ota-atlas/site` (book atlas), copied in by `ota-atlas/tools/publish.sh` |
| `pasiaj.com/ota/sim/` | same static nginx | `ota-engine/viewer` (onboarding, docs, live views), copied in by `ota-engine/tools/publish.sh` |
| `pasiaj.com/ota/api/` | new Deployment `ota-sim` in namespace `xpost` (servermac OrbStack k8s), Ingress path `/ota/api` on host pasiaj.com, TLS `static-tls` | `louhi-games` image `ota-sim:prod` built on servermac (x86_64) |

- **Data**: PVC `ota-data` (local-path). On first start, the container runs `louhi history` (deterministic, days 1–365) when the store is empty.
- **OpenAI**: the existing Secret `openai` (key `api_key`) is mounted as `OPENAI_API_KEY`.
- **Run control** is public read-only. `POST /api/run` needs the `X-OTA-Admin` token from Secret `ota-admin`, so strangers can't spend tokens. The viewer asks for the token once and keeps it in localStorage.
- **Autopilot**: by default the server advances one simulated day per real day at 03:00 Europe/Helsinki, under the daily budget cap. So the company "keeps living".
- The manifests live in `xpost-cluster/base/ota-sim/`, and its post-receive hook applies them. Image builds use `louhi-games/deploy/build-on-servermac.sh`, run over ssh, because the xpost hook only builds xpost images.

---

## 13. Amendments from the validation review (these override earlier sections)

An independent review checked this plan against the book text. These amendments **take precedence** over §1–§12.

**Chapter references.** Ch 13 is the toolkit overview, ch 14 observation, ch 15 thinking, ch 16 action, and ch 17 the human tasks and the constitution. Decision Architecture is ch 16. The three human tasks are ch 17.

### Must-fix

1. **`AiFailureMode` comes from the book's 13 modes** listed in ota.js:
   - Bypass, Abdication, Synthetic Certainty, Alert Fatigue, AI Echo Chamber, Homogenisation, Decision Laundering, Diffusion of Accountability, Glass Cockpit, Assumption Graveyard, Learning-loop Inversion, Epistemic Corruption, …
   - Use exactly the list in ota.js `failure_modes`.

   Corrected signatures:
   - **Glass cockpit** means skills wasting away. Its signature is a falling share of "manual" cycles, where a human reads the raw data before the AI summary or compares the system's answer with an expert's.
   - **Abdication** is rubber-stamping. Its signature is a human approval time below a minimum for high-stakes items, or human reasons that copy the LLM rationale.
   - **Decision laundering** is the gap `committed_ts − premortem_started_ts` falling below a minimum (ch 15). Store both timestamps.
   - **Assumption graveyard** is a growing count of stale assumptions with no archive or renew decision.
   - **Learning-loop inversion** is double-loop recommendations that keep recurring while the share of external signals shows a regime shift.
   - **Epistemic corruption** is calibration scores drifting in favour of the scored party. The independent audit checks it.
2. **Sentinel signal classes** come in addition to the pathogens: calibration drift, capability decay, falling dissent and decaying metrics (ch 16).
3. **Domain Expert Gate**:
   - Yellow records the recommendation and the decision **proceeds**.
   - Red **blocks** until an independent expert review is recorded.
   - A human (the validator persona) defines the classification and the system applies it automatically.
   - If the AI holds Path 2 authority for the class, it may perform the review, except for irreversible classes marked `human_only`.
4. **Autonomy**:
   - Path 1 classes (reversible, low impact) default to `ActThenReview` or `Autonomous`, with humans reviewing patterns afterwards. They don't have to earn this.
   - Path 2 needs a narrow domain and proven results.
   - Authority is reviewed **monthly for Path 2 classes and quarterly for Path 1**, plus off-cycle when thresholds are crossed. This replaces the weekly review.
   - The evidence is the Brier score plus the **agreement rate with human judgments** on the same inputs.
   - **[deviation]** The book's ladder has no named rungs; the five-rung enum is ours.
5. **Calibration privacy**: individual (human) calibration profiles are visible only to that person. The viewer shows aggregate and per-AI-module calibration. Sentinel receives aggregates only.
6. **Scenario tree tiers** are `major` (the AI proposes, leadership approves at the monthly meeting), `minor` (the AI adds it with a log entry, reviewed at the next review) and `urgent` (an off-schedule approval request to a named decider).
7. **Layered rollout in the history**: the toolkit is switched on in stages over the simulated year:
   - Ledger: from day 1
   - Assumption Cycle: month 2
   - Adversarial Engine and Auto Pre-Mortem: month 3
   - Lattice, Signal Brief and Scenario Tree: month 4
   - Calibration, Autonomy, Gate, OKRs and Coherence: month 5–6
   - Sentinel and Double-loop: month 7
   - Meta-loop and Independent audit: month 9

   Early flags are noisy, because the ledger is young (ch 15). **Backstory**: before day 1, the founders ran human-driven pre-mortems, red teams and AARs for a year at the previous studio era (ch 13 says the human versions must come first). The `Tool.enabled_from` day is in `toolkit.md`.
8. **The Lattice has six levels**. L6 is human judgment, or the decider step when autonomy has been earned. Deviation records carry `level` up to 6.

### Additions

9. **The four missing elements of action** (ch 4) are required on prototype, soft-launch and scale decisions: a stated hypothesis, a learning metric, a named decision point, and reversibility. **Decision latency** is the number of days from the first Lattice deviation (or anomaly cluster) to the ledger commit that cites it. **Pilot limbo** is a Sentinel rule (more than N CPI tests without go/kill). **Big-bang** is a Sentinel rule (scale-up without a test).
10. **Truth pipeline** (ch 11):
    - Each strategic signal domain (UA cost, platform policy, competitors, player sentiment) has a **metric owner** with ≥3 independent sources, plus a monthly situational-awareness review.
    - The **shadow group** is the end-node humans. It reports directly to leadership and rotates every 6 months.
11. **Anomaly journal** (the observation register): an `Anomaly {id, day, by, text, provenance, cluster}` record that end nodes and modules write when something feels off below any threshold. L3 promotes a cluster when ≥3 independent sources agree.
12. **Grid zones and archetypes**: the quarterly grid tags the universal anchor (thinking × direction) and the culture trap. Louhi is classified against the archetypes. **Residual risk** is shown moving across cells as tools come online.
13. **The Meta-loop** reads external evidence (an environment "research and peer practice" feed) and Sentinel's own performance. It **only proposes**; leadership approves.
14. **The Strategy Coherence regulatory check**: ClaimNode kinds include `regulation` (app-store policy, GDPR, ATT consent, loot-box rules, age ratings).
15. **Alert dismissal is an explicit, logged act** by the recipient, not a silent suppression. Over-budget alerts queue with "dismiss or act" required.
16. **Nice to have**:
    - learning rate (Wright curve) on CPI-test cost and prototype cycle time
    - loop speed separated from throughput
    - a `commitment_point` field on scale decisions
    - a dissent log derived from rejected challenges plus overrides
    - a leader think week once a quarter (capacity blocked)

### Org-model caveats

17. **[deviation, the largest]** The three human tasks (framing, judgment, accountability, ch 17) are performed by LLM stand-ins. That is exactly the homogenisation risk the book warns about.
    - Mitigation: a real human can override any framing through live "human input" in the viewer (admin token).
    - Every framing and approval record carries `source: real_human | standin`.
    - The viewer shows the real-human share prominently.
18. **End nodes are decision-makers and experts** (ch 17), not task executors. They own some Path 1 classes, write anomalies, raise challenges and form the shadow group.
19. **Culture cannot be generated by the tools** (ch 16). Psychological safety comes from the persona inputs (bias profiles). A constitution principle 2 metric counts bad news raised by humans and the response it got.
