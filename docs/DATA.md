# Data contract

This is the contract the viewer is built against. Everything the engine writes is plain
JSON under the data directory, and the HTTP API (`ota-server`) serves the same shapes.
Field names are `snake_case`; enums serialise as `snake_case` strings; optional values
are `null` (never omitted) unless stated. Numbers are JSON numbers (f64 unless noted).

Source of truth for the Rust types: `crates/ota-core/src/records.rs` and
`crates/ota-core/src/enums.rs`. Example values below come from the tiny example company
(`examples/tiny`).

## 1. Layout

```
data/
  meta.json                 static company description (§3)          → GET /meta
  days/D0001.json           one complete DayRecord per day (§4)      → GET /day/:d
  state/view.json           public state snapshot (§6)               → GET /state
  state/ledger.json         full Decision Ledger {"decisions": {id: Decision}}  → GET /ledger, /decision/:id
  state/private/<person>.json  private human calibration (§6.3)      → GET /calibration/person/:id (admin)
  state/*.json              other internal tool state (not a contract; may change)
  log/kpis.jsonl            one line per day: KPIs + ota.* metrics (§7) → GET /series
  log/ledger.jsonl          ledger events (§7)
  log/llm.jsonl             every LLM call (§7)
  log/human.jsonl           every HumanAction (§7)
  inbox/*.json              pending real-human inputs (consumed by the next tick)
  run_status.json           background run progress (§8)             → GET /run/status
  llm-cache/                LLM response cache (opaque)
```

Day files are written atomically at the end of each tick. `state/*.json` are written at
checkpoints (every 30 days and at the end of every run), so during a long run the
ledger/state may lag the newest day file by up to 30 days.

## 2. Conventions

### Ids

`<PREFIX>-<DDDD>-<NNNN>`: prefix, the 4-digit day the record was created, a per-day
sequence. Unique across the run.

| Prefix | Record | Prefix | Record |
|---|---|---|---|
| `D` | Decision | `DV` | Deviation |
| `S` | Signal | `E` | EnvEvent (unless the world supplies one) |
| `M` | Message | `F` | Forecast |
| `PM` | PreMortem | `CH` | Challenge |
| `A` | Action | `H` | HumanAction |
| `Q` | QueueItem | `AU` | AutonomyChange, also audit Finding |
| `SN` | SentinelFinding | `AN` | Anomaly |
| `SC` | ScenarioNode proposed by the engine | `CO` / `DL` / `ML` | Finding (coherence / double loop / meta-loop) |

Seeded scenarios, assumptions (`A-…`) and OKRs keep the ids from the company spec.

### Node ids

Messages, `routed_to`, `by`, `who`, `person` etc. hold **node ids**: module ids
(`ua`, `analytics`), person ids (`ceo`), or engine pseudo-nodes for toolkit steps:
`reality-lattice`, `assumption-cycle`, `living-okrs`, `domain-expert-gate`,
`double-loop`, `sentinel`, `signal-brief`, `adversarial-engine`, `meta-loop`, `scenario-tree`.
A company may also define toolkit modules (MODULE.md with `tool:`) — those get a
`ModuleRun` each day (see §4.5).

### Time

`day` is the 1-based simulation day; `date` is `YYYY-MM-DD` (day 1 = `company.start_date`).
`ts` is the minute of the simulated day (0–1439, the clock starts at 480 = 08:00).

### Enums

| Enum | Values |
|---|---|
| Phase | `observe think act` |
| Modality | `direction structure processes capability culture` |
| Pathogen | `hubris complacency normalisation_of_deviance groupthink misleading_incentives inertia` |
| AiFailureMode (13) | `bypass abdication synthetic_certainty alert_fatigue ai_echo_chamber decision_laundering assumption_graveyard glass_cockpit learning_loop_inversion diffusion_of_accountability homogenisation epistemic_corruption cultural_boundary` |
| SentinelClass | `calibration_drift capability_decay falling_dissent decaying_metrics` |
| Reversibility | `reversible costly_to_reverse irreversible` |
| Impact | `local cross_function company_wide` |
| Rung (ordered) | `inform recommend act_with_approval act_then_review autonomous` |
| GateColor | `green yellow red` |
| Tier | `rule small large human` |
| DecisionPath | `path1 path2` |
| DecisionKind | `routine test go_kill launch scale` |
| DecisionStatus | `proposed approved rejected executing sealed reverted blocked deferred` |
| Provenance | `internal external` |
| HumanSource | `real_human standin` |
| NodeKind | `leader ai_module toolkit validator end_node auditor` |
| Intent | `inform request propose challenge escalate approve reject task report` |
| AssumptionStatus | `active confirmed refuted stale archived` |
| ScenarioTier | `major minor urgent` |
| ClaimKind | `mission strategy okr budget incentive policy regulation` |
| Cadence | `daily weekly monthly quarterly` |
| HumanVerdict | `approve reject amend defer` |
| QueueKind | `approval gate_review review task framing autonomy_change scenario_approval brief_approval meta_loop_proposal` |
| Severity | `low medium high` |
| FindingStatus | `open acknowledged resolved` |

Decision status lifecycle: `proposed` → (`blocked` at a red gate → `proposed`) →
`approved` → `executing` (action applied, or `hold`) → `sealed` (outcome on the check
date). Side exits: `rejected`; `reverted` when a later stop decision undoes it.

## 3. `meta.json` — `GET /meta`

```json
{
  "engine_version": "0.1.0",
  "company": {"id": "tiny", "name": "Tiny Games Oy", "start_date": "2026-01-05", "timezone": "Europe/Helsinki",
              "currency": "EUR", "premortem_threshold": 150.0, "alert_budget": 3, "learning_curve_kpis": ["cpi"],
              "pilot_limbo_n": 3, "description": "markdown…", "extra": {"…all company.md front-matter keys": "…"}},
  "constitution": [{"n": 1, "text": "Every decision … is a hypothesis.", "enforced_by": "ledger switches"}],
  "toolkit": [{"id": "ledger", "enabled_from": 1, "depends_on": [], "evolves_from": "decision record", "human_checkpoint": null}],
  "modules": [{"id": "ua", "name": "User acquisition", "org": "growth", "kind": "ai_module", "phases": ["think", "act"],
               "cadence": "daily", "inputs": ["cpi", "roas_d7"], "outputs": ["bid"], "owner": "ua", "accountable": "ceo",
               "evolves_from": "UA manager", "depends_on": ["analytics"], "alert_budget": 2, "modality": "processes",
               "tool": null, "metric_owner_of": ["ua_cost"], "classes": ["ua.bid_adjust"], "description": "markdown…",
               "prompts": ["think"]}],
  "people": [{"id": "ceo", "name": "Aino Example", "role": "leader", "title": "CEO", "accountable_for": ["ua"],
              "expertise": ["strategy"], "llm_tier": "human", "weekly_capacity_min": 900.0,
              "bias": {"overconfidence": 0.2, "challenge_acceptance": 0.6, "review_diligence": 0.8},
              "shadow_group": false, "persona": "markdown…", "judgment": "markdown…",
              "gate_rules": [{"classes": ["ua.*"], "red_if": "stakes_eur >= 2000 && novelty > 0.5", "yellow_if": "stakes_eur >= 500"}]}],
  "classes": [{"id": "ua.bid_adjust", "module": "ua", "accountable": "ceo", "reversibility": "reversible", "impact": "local",
               "path": "path1", "default_rung": "act_then_review", "stakes_eur": 300.0, "novelty": 0.2, "kind": "routine",
               "modality": null, "human_only": false, "validator": null, "assumptions": ["A-roas"],
               "forecast": "D7 ROAS …", "confirm_if": "roas_d7 >= 1.0", "stop_if": "roas_d7 < 0.7", "check_after_days": 7,
               "rows": [{"when": "roas_d7 > 1.15 && spend_util > 0.9", "then": "bid *= 1.10", "confidence": 0.72, "escalate": null, "note": null}],
               "description": "…"}],
  "kpis": [{"metric": "cpi", "target": 2.0, "direction": "down", "unit": "EUR", "owner": "ua", "description": "…", "domain": "ua_cost"}],
  "llm": {"daily_budget_usd": 0.5, "fallback": ["large", "small"],
          "tiers": {"small": {"provider": "openai", "model": "gpt-4.1-nano", "base_url": "https://api.openai.com/v1", "price_in": 0.1, "price_out": 0.4}}},
  "llm_mode": "scripted",
  "scripted_until": 0,
  "warnings": [],

  "days": {"first": 1, "last": 120, "count": 120},
  "run": {"running": false, "current_day": 120, "llm_mode": "scripted"},
  "live_llm_available": false,
  "admin_enabled": true,
  "autopilot": {"hour": 3, "timezone": "Europe/Helsinki", "days": 1}
}
```

The last five keys are added by the server (not in the file). `llm` never contains keys
or key variable names. `llm_mode` is the mode of the engine that last wrote the file;
each day record carries its own `llm_mode`.

## 4. Day record — `days/DNNNN.json`, `GET /day/:d`

```json
{
  "day": 12, "date": "2026-01-16", "weekday": "fri",
  "cadences": ["daily"],                 // + "weekly" (Mon), "monthly" (1st), "quarterly" (Jan/Apr/Jul/Oct 1st)
  "tools_enabled": ["ledger", "assumptions", "..."],   // tool ids switched on today (layered rollout)
  "llm_mode": "scripted",                // "live" | "scripted"
  "events": [EnvEvent],
  "signals": [Signal],
  "kpis": {"cpi": 3.15, "roas_d7": 0.71, "dau": 41210.6},
  "deviations": [Deviation],             // detected today (with today's disposition, if any)
  "deviation_updates": [Deviation],      // earlier/over-budget alerts disposed today (full record)
  "anomalies": [Anomaly],
  "assumption_updates": [AssumptionUpdate],
  "module_runs": [ModuleRun],
  "messages": [Message],
  "decisions": [Decision],               // created today; state as of end of day
  "decision_updates": [Decision],        // older decisions changed today (approved, sealed, reverted…); full record
  "premortems": [PreMortem],
  "challenges": [Challenge],             // created today, plus older ones overridden/accepted today
  "forecasts_made": [Forecast],
  "forecasts_resolved": [Forecast],
  "actions": [Action],
  "human_actions": [HumanAction],
  "human_load": {"ceo": 99.4},           // minutes spent today per person (only people who worked)
  "queue_open": {"ceo": 1, "ua-scientist": 0},   // open queue items per person at end of day
  "autonomy_changes": [AutonomyChange],  // proposed or resolved today
  "sentinel": [SentinelFinding],         // raised today (incl. audit's epistemic-corruption findings)
  "findings": [Finding],                 // double loop, coherence, audit, meta-loop
  "brief": SignalBrief | null,
  "scenario_changes": [ScenarioNode],    // nodes proposed, re-weighted, approved or rejected today
  "okr_flags": [OkrState],               // OKRs flagged to their setter today
  "grid": GridScore | null,              // quarterly (and day 1)
  "llm_calls": [LlmCallRecord],
  "metrics": DayMetrics,
  "log": ["day 12 done in 3 ms"]
}
```

(`//` comments are documentation only; the files are strict JSON.)

To get *all* versions of a decision, use `GET /decision/:id` or the ledger; a day file
holds the state at the end of that day.

### 4.1 EnvEvent

```json
{"id": "E-0012-0001", "day": 12, "kind": "platform_policy", "title": "Platform privacy change raises CPIs",
 "text": "Attribution signal loss across ad networks.", "effects": {"cpi": 1.45, "half_life_days": 25},
 "tags": ["privacy"], "generated_by": "storyline"}
```
`effects` is free-form JSON defined by the company world. `generated_by`: `llm`, `scripted`, `storyline`, ….

### 4.2 Signal

```json
{"id": "S-0012-0006", "day": 12, "source": "telemetry", "kind": "kpi", "metric": "roas_d7", "value": 0.71,
 "text": "", "provenance": "internal", "tags": []}
```
Every KPI becomes a `telemetry` signal; every event a signal with `source: "event:<id>"`,
`provenance: "external"`; the world may add others (reviews…).

### 4.3 Deviation (Reality Lattice)

```json
{"id": "DV-0012-0002", "day": 12, "metric": "cpi", "value": 3.15, "baseline": 2.06, "sigma": 0.048, "z": 22.5,
 "level": 6, "detector": "z",
 "interpretation": "cpi moved +22.5σ; check external events before acting.",
 "routed_to": ["ua"], "suppressed_by_budget": false,
 "disposition": {"day": 12, "by": "ua", "action": "acted", "reason": "covered by D-0012-0001", "decision": "D-0012-0001"}}
```

* `level` = highest lattice level reached: 1 coverage gap only, 3 detected, 4 interpreted
  (L4 LLM), 5 routed (L5), 6 a committed decision cites it (the decider, L6).
* `detector`: `z` (|z| ≥ 2 vs 28-day weekday-adjusted baseline), `trend_break`,
  `coverage_gap` (expected metric missing; weekly), `anomaly_cluster`
  (metric `anomaly:<cluster>`, `value` = entries, `z` = independent sources; routed to leaders).
* `suppressed_by_budget: true` → no recipient had alert budget left; the alert is queued
  and must later be explicitly `dismissed` or `acted` (`disposition`, reported in a later
  day's `deviation_updates`). Alert fatigue is therefore visible.
* `disposition.action`: `acted` | `dismissed`; `null` until disposed.

### 4.4 Anomaly (observation register)

```json
{"id": "AN-0024-0001", "day": 24, "by": "ceo", "text": "Players report slow loading after the update",
 "provenance": "internal", "cluster": "loading", "tags": ["loading"]}
```

### 4.5 ModuleRun — the central record (`GET /day/:d/module/:m`)

*For this module, on this day: these were the inputs, this is how it thought, these were the outputs.*

```json
{
  "day": 12, "module": "ua",
  "inputs": {
    "deviations": ["DV-0012-0002"],             // routed today within budget
    "queued_alerts": ["DV-0012-0006"],          // over-budget alerts it must dismiss or act on
    "messages": ["M-0011-0004"],                // inbox delivered since its last think
    "due_switches": ["D-0008-0001"],            // own decisions sealed today
    "assumption_flags": ["A-roas"],             // own assumptions that changed today
    "okr_flags": [],                            // flagged OKRs whose KRs it reads
    "tasks": [],
    "metrics": {"cpi": 3.15, "roas_d7": 0.71}   // snapshot of its MODULE.md inputs
  },
  "think": {
    "tier": "small",                            // highest tier used: rule | small | large | human
    "rules_fired": [{"class": "ua.bid_adjust", "row": 1, "when": "roas_d7 < 0.80", "then": "bid *= 0.85",
                     "confidence": 0.7, "escalate": null, "bindings": {"roas_d7": 0.708}}],   // row is 0-based
    "rules_failed": [],                         // unevaluable rows, unparseable LLM actions, deferrals
    "prompt_ref": "modules/ua/prompts/think.md",
    "llm_model": "gpt-4.1-nano",                // "scripted" when scripted
    "llm_request": "Day 12 … Escalated items: […]",
    "llm_response": "{\"rationale\": …, \"decisions\": […]}",
    "llm": LlmCallRecord | null,
    "fallback": "large (budget) → small" | null,
    "rationale": "human-legible summary of how it thought"
  },
  "outputs": {"decisions": ["D-0012-0001"], "actions": [], "messages": [], "forecasts": [],
              "dismissed_alerts": ["DV-0012-0005"], "anomalies": []},
  "cost_usd": 0.0004, "tokens": 1830, "duration_ms": 1
}
```

* `bindings` keys are identifiers as written (`roas_d7`, `z(cpi)`, `trend(cpi)`) with the
  values used at evaluation time.
* `llm_*` fields are `null` when no escalation happened (most days for most modules).
* Toolkit modules (MODULE.md with `tool:`) get a ModuleRun summarising their tool's
  activity: `think.tier = "rule"`, `think.rationale = "[lattice] 6 deviation(s); …"`,
  and relevant ids in `outputs`/`inputs.deviations`.
* `actions` in outputs is filled by the act step only through `Decision.actions`;
  join `actions[].decision` with `outputs.decisions` to show them.

`GET /day/:d/module/:m` returns:

```json
{"day": 12, "date": "2026-01-16", "module": "ua", "run": ModuleRun | null,
 "messages_in": [Message], "messages_out": [Message], "decisions": [Decision],
 "decision_updates": [Decision], "deviations": [Deviation], "actions": [Action]}
```
404 if the module had no run and no messages/decisions that day.

### 4.6 Decision (Decision Ledger entry)

```json
{
  "id": "D-0012-0001", "class": "ua.bid_adjust", "module": "ua",
  "what": "ua.bid_adjust: bid *= 0.85",          // ① what
  "when": 12,                                    // ② when (day proposed)
  "who": "ua",                                   // ③ who decided: module id, or the approving person
  "information": ["DV-0012-0002", "S-0012-0006"],// ④ information: deviation / signal / decision ids
  "assumptions": ["A-roas"],                     // ⑤ assumptions
  "expected": "D7 ROAS of the adjusted bid ≥ 1.0 in 7 days",   // ⑥ expected outcome
  "switches": {"confirm_if": "roas_d7 >= 1.0", "stop_if": "roas_d7 < 0.7", "check_by": 19},  // ⑦ decision switches
  "action": {"kind": "adjust", "params": {"target": "bid", "op": "*", "value": 0.85}},
  "kind": "routine", "reversibility": "reversible", "impact": "local", "path": "path1",
  "stakes_eur": 300.0, "novelty": 0.4, "confidence": 0.665,
  "forecasts": ["F-0012-0001"],
  "tier": "small",                               // tier that produced the proposal
  "rung_at_time": "act_then_review",
  "reasoning": "human-legible reasoning (multi-line)",
  "alternatives": ["bid *= 1.10", "hold"],
  "premortem": "PM-0012-0001" | null,
  "premortem_started_ts": 491 | null,            // minute the pre-mortem started
  "committed_ts": 537 | null,                    // minute of commitment (approval or auto-execute)
  "committed_day": 12 | null,
  "challenges": ["CH-0012-0001"],
  "gate": GateReview | null,                     // null before the gate tool is enabled
  "approvals": [Approval],
  "overrides": [Override],
  "accountable": "ceo",                          // named human, always present
  "status": "executing",
  "outcome": Outcome | null,
  "action_elements": {"hypothesis": "…" | null, "learning_metric": "roas_d7" | null,
                      "decision_point": "…" | null, "reversibility": "costly_to_reverse — can be undone at a cost" | null},
  "commitment_point": "…" | null,
  "informed": ["ceo"],                           // who was informed (impact classification)
  "actions": ["A-0012-0001"],
  "first_signal_day": 12 | null,                 // earliest cited deviation (decision latency)
  "notes": ["stops D-0005-0002", "amended by ceo: …", "missing action elements: hypothesis"]
}
```

* Decision laundering gap = `committed_ts − premortem_started_ts` (minutes, same day).
* A stop decision created when a stop signal fires has `action.kind = "revert"`,
  `action.params = {"decision": "<original id>", "action": {...}}` and a note `stops <id>`.

**GateReview**
```json
{"color": "red", "rule": "ua-scientist: red_if stakes_eur >= 2000 && novelty > 0.5", "validator": "ua-scientist",
 "reviewed_by": "ua-scientist" | "ai" | null, "reviewed_day": 14 | null, "verdict": "approve" | null,
 "recommendation": "validator notified; decision proceeds" | null}
```
Yellow proceeds (validator informed by message); red blocks (`status: "blocked"`) until
reviewed. `reviewed_by: "ai"` means the AI reviewed under earned Path 2 authority.

**Approval** (also post-hoc reviews: `post_hoc: true`)
```json
{"by": "ceo", "day": 13, "ts": 545, "verdict": "approve", "reason": "…", "minutes_spent": 12.5,
 "source": "standin", "amendments": null, "post_hoc": false}
```

**Override**
```json
{"by": "ceo", "day": 13, "what": "challenge:CH-0012-0001", "reason": "…", "source": "standin"}
```
`what`: `challenge:<id>`, `post_hoc_review`, `real_human_<verdict>`.

**Outcome** (sealed on `switches.check_by`)
```json
{"sealed_day": 19, "actual": "confirm signal hit: roas_d7 >= 1.0", "expected": "…", "deviation": "as expected",
 "success": true, "confirm_hit": true, "stop_hit": false, "metrics": {"roas_d7": 1.04}}
```
`success`: `true` (confirm, no stop), `false` (stop hit), `null` (inconclusive).

### 4.7 PreMortem

```json
{"id": "PM-0008-0001", "decision": "D-0008-0001", "day": 8, "started_ts": 486,
 "failure_modes": [{"text": "costs rise faster than revenue", "p": 0.3, "severity": 0.6}],
 "load_bearing": ["A-roas"], "skipped": false, "skip_reason": null, "tier": "large"}
```
Skipped pre-mortems are recorded with `skipped: true` and `skip_reason`
(`"auto pre-mortem not enabled yet (layered rollout)"`, `"LLM budget exhausted"`).

### 4.8 Challenge (Adversarial Engine)

```json
{"id": "CH-0012-0001", "day": 12, "target": "A-roas", "decision": "D-0012-0001",
 "argument": "…", "sources": ["network dashboard"], "strength": 0.54,
 "response": "ua: noted; stop signal `roas_d7 < 0.7` covers this risk",
 "overridden_by": "ceo" | null, "accepted": true | false | null}
```

### 4.9 Forecast

```json
{"id": "F-0012-0001", "day": 12, "by": "ua", "question": "D7 ROAS … in 7 days", "p": 0.665, "resolves_on": 19,
 "outcome": null, "brier": null, "topic": "ua.bid_adjust", "horizon_days": 7, "decision": "D-0012-0001",
 "resolve_if": "roas_d7 >= 1.0", "human": false}
```
Human forecasts (`human: true`, `topic: "approval"`) are never in day files; they feed
private profiles only.

### 4.10 Action

```json
{"id": "A-0012-0001", "decision": "D-0012-0001", "module": "ua", "kind": "adjust",
 "params": {"target": "bid", "op": "*", "value": 0.85}, "executed_day": 12, "effect_summary": "bid: 2.42 → 2.06"}
```

### 4.11 Message

```json
{"id": "M-0012-0001", "day": 12, "ts": 486, "from": "ua", "to": ["ceo"], "intent": "inform",
 "subject": "Committed: ua.bid_adjust: bid *= 0.85", "body": "…", "refs": ["D-0012-0001"], "hops": 1}
```
`hops`: nodes the information passed through before reaching `to` (0 = a human's own
framing; 1 = straight from the AI layer). Messages are written on the day they are sent.

### 4.12 HumanAction

```json
{"id": "H-0012-0001", "day": 12, "ts": 509, "person": "ceo", "item": "Q-0011-0002", "kind": "approval",
 "decision": "D-0011-0003" | null, "verdict": "approve", "reason": "…", "amendments": null,
 "minutes_spent": 23.2, "confidence": 0.79, "source": "standin", "manual_check": false}
```
`source: "real_human"` for live viewer input. `manual_check`: the person read raw data
before the AI summary (glass-cockpit counter). `kind: "framing"` records a real human's
framing message (`item` is then the message id).

### 4.13 AssumptionUpdate

```json
{"assumption": "A-roas", "from": 0.65, "to": 0.52, "status": "active", "reason": "refuted by roas_d7 < 0.8"}
```

### 4.14 AutonomyChange

```json
{"id": "AU-0022-0001", "day": 22, "class": "ua.bid_adjust", "from": "act_then_review", "to": "act_with_approval",
 "evidence": {"brier": 0.37, "n": 4, "agreement_rate": 0.0, "threshold": 0.3},
 "trigger": "off_cycle", "status": "approved", "approved_by": "ceo", "source": "standin"}
```
`trigger`: `monthly_review` (Path 2), `quarterly_review` (Path 1), `off_cycle` (weekly
check, demotions only). `status`: `proposed` → `approved` | `rejected`. A change appears
in the day it was proposed and again in the day it was resolved.

### 4.15 SentinelFinding

```json
{"id": "SN-0030-0001", "day": 30,
 "kind": {"category": "failure_mode", "name": "alert_fatigue"},
 "signature": "3 alerts over budget (33%); acknowledgement latency 0.0 d",
 "stats": {"suppressed_share": 0.33, "ack_latency": 0.0},
 "evidence": ["DV-0012-0006"], "severity": "medium", "status": "open"}
```
`kind.category`: `pathogen` (name ∈ Pathogen), `failure_mode` (∈ AiFailureMode),
`signal_class` (∈ SentinelClass), `rule` (`pilot_limbo`, `big_bang`,
`missing_action_elements`). A kind re-fires at most once per 7 days.
`epistemic_corruption` is raised by the monthly independent audit (separate code path).

### 4.16 Finding

```json
{"id": "DL-0035-0001", "day": 35, "tool": "double_loop", "kind": "recurring_failure",
 "text": "ua.bid_adjust: 3/4 outcomes failed in 28 days. …", "refs": ["D-0020-0001"],
 "stats": {"sealed": 4, "failed": 3, "weeks_recurring": 1}}
```
| tool | kinds |
|---|---|
| `double_loop` | `recurring_failure` |
| `coherence` | `budget_gap`, `regulation_gap`, `okr_unsupported`, `dangling_link` |
| `audit` | `sentinel_integrity`, `accountability`, `instrument_audit` |
| `meta_loop` | `proposal`, `proposal_approve`, `proposal_reject`, `proposal_amend` |

### 4.17 SignalBrief

```json
{"day": 12, "headline": "…", "items": ["…"], "tier": "small", "approved_by": "ceo" | null, "source": "standin" | null}
```
The brief is drafted on its day; approval happens in a later tick and is visible in
`state/view.json` → `briefs`.

### 4.18 ScenarioNode

```json
{"id": "SC-0073-0004", "parent": [], "title": "Regime shift: installs down (-4.2σ)", "p": 0.11,
 "leading_indicators": [{"expr": "abs(z(installs)) > 2", "effect": "confirm"}], "assumptions": [],
 "tier": "minor", "status": "logged", "approved_by": null, "proposed_by": "scenario-tree", "created": 73,
 "history": [{"day": 73, "confidence": 0.2, "reason": "proposed (minor)"}]}
```
`status`: `approved` (seeded / approved), `pending` (major/urgent awaiting a leader),
`logged` (minor, added with a log entry), `rejected`. `parent` lists parents (DAG).
`history[].confidence` is the probability `p`.

### 4.19 OkrState

```json
{"id": "O1", "objective": "Grow the game profitably",
 "key_results": [{"metric": "dau", "target": 50000.0, "current": 41210.6, "direction": "up"}],
 "assumptions": ["A-retention"], "setter": "ceo",
 "flagged": "dau at 41210.6 vs target 50000.0" | null, "flagged_day": 12 | null}
```

### 4.20 GridScore (quarterly success-grid self-assessment)

```json
{"quarter": "2026-Q2", "day": 87,
 "cells": [[1.0, 0.75, 0.75, 0.5, 0.66], [1.0, 0.75, 1.0, 0.75, 0.53], [0.75, 0.75, 0.49, 0.75, 0.44]],
 "residual_risk": [[0.0, 2.5, 3.1, 1.6, 4.9], [0.0, 13.0, 0.0, 1.9, 34.9], [5.4, 4.7, 13.5, 0.8, 13.8]],
 "universal_anchor": 1.0, "culture_trap": 0.54,
 "nearest_profile": "successes",
 "profile_distance": {"failures_all": 90.6, "failures_nonfraud": 93.4, "failures_fraud": 86.1, "successes": 83.0},
 "archetype": "Growth-company founder (Kasvuyrityksen perustaja)", "notes": []}
```
Rows = phase `[observe, think, act]`; columns = modality `[direction, structure,
processes, capability, culture]`. `cells` are health 0–1; `residual_risk` is the OTA 200
non-fraud failure weight × weakness, normalised to 100. `universal_anchor` = think ×
direction cell; `culture_trap` = mean of the culture column. `profile_distance` = L1
distance between our weakness profile and the OTA 200 weight profiles (lower = closer).
`archetype` ∈ the three ota.js archetypes or `balanced`.

### 4.21 LlmCallRecord

```json
{"tier": "small", "requested_tier": "large", "model": "gpt-4.1-nano", "provider": "openai", "purpose": "module_think",
 "tokens_in": 1200, "tokens_out": 630, "cost_usd": 0.00037, "cached": false,
 "fallback": "large (budget) → small" | null, "error": null}
```
`purpose`: `module_think`, `lattice_interpret`, `premortem`, `adversarial`, `standin`,
`brief`, `meta_loop`, `env_events` (and any company-defined purpose). `tier: "rule"` with
`provider: "none"` means the budget forced a fall back to rules. `error` set + 
`provider: "scripted"` means a live call failed and the scripted responder answered.

### 4.22 DayMetrics

```json
{"decisions_committed": 1, "decisions_by_tier": {"rule": 1}, "decision_latency_days": 0.0, "loop_speed_days": 0.0,
 "throughput_per_day": 1.0, "learning_rate": 1.07, "hops_to_decider": 1.0, "real_human_share": 0.0,
 "external_signal_share": 0.5, "human_minutes": 37.7, "alerts_routed": 2, "alerts_suppressed": 0,
 "premortems_run": 0, "premortems_skipped": 0, "ledger_completeness": 0.27, "brier_30d": 0.34,
 "llm_cost_usd": 0.0, "bad_news_raised": 0,
 "extra": {"coherence_gap": 0.3, "queue_open": 1.0, "stale_assumptions": 0.0}}
```
* `decision_latency_days`: mean days from the first cited deviation to commit, for
  decisions committed today. `loop_speed_days`: mean days from first signal (or proposal)
  to execution for today's actions (loop speed, separate from `throughput_per_day`).
* `learning_rate`: Wright progress ratio on `company.learning_curve_kpis` (cost multiplier
  per doubling; < 1 = learning).
* `hops_to_decider`: mean hops of propose/escalate/request messages to humans today.
* `real_human_share`: real-human share of human actions over the trailing 30 days.
* `external_signal_share`: external share of non-telemetry signals today.
* `ledger_completeness`: share of last-30-day decisions with all seven fields filled.
* `brier_30d`: AI forecasts resolved in the last 30 days.
* Optional values are `null` when undefined for the day.

## 5. Ledger — `GET /ledger`, `GET /decision/:id`

`GET /ledger?q=&class=&module=&status=&from=&to=&limit=` (all optional; `class` accepts
a trailing `*`; `q` matches id, class, `what`, `reasoning`; default limit 200, newest first):

```json
{"total": 36, "count": 7, "decisions": [Decision]}
```

`GET /decision/:id`:

```json
{"decision": Decision, "premortem": PreMortem | null, "challenges": [Challenge],
 "actions": [Action], "forecasts": [Forecast], "assumptions": [Assumption]}
```
(premortem/challenges/actions/forecasts from the decision's own day; actions executed
on later days are listed by id in `decision.actions`.)

## 6. Public state — `state/view.json`, `GET /state`

```json
{
  "day": 120, "date": "2026-05-04",
  "toolkit": [{"id": "sentinel", "enabled_from": 10, "enabled": true, "depends_on": ["lattice", "calibration"], "evolves_from": null}],
  "assumptions": [Assumption],
  "graveyard": ["A-…"],                       // stale assumptions with no archive/renew decision
  "scenarios": [ScenarioNode],
  "autonomy": {"rungs": {"ua.bid_adjust": "act_with_approval"}, "history": [AutonomyChange]},
  "calibration": PublicCalibration,
  "okrs": [OkrState],
  "sentinel": [SentinelFinding],              // newest first, up to 300
  "findings": [Finding],                      // last 300
  "coherence_gap": 0.3 | null,
  "grid": [GridScore],                        // one per quarter, ascending
  "briefs": [SignalBrief],                    // newest first, up to 14
  "queue": [QueueItem],                       // all open human queue items
  "alert_queue": {"ua": 2},                   // over-budget alerts awaiting disposition per node
  "anomaly_clusters": {"loading": 26},        // cluster → day promoted to L3
  "double_loop_recurring": {"ua.bid_adjust": 2},   // class → consecutive weeks flagged
  "ledger_size": 36,
  "llm_spent_total_usd": 0.0                  // spend of the engine instance that wrote the file
}
```

### 6.1 Assumption

```json
{"id": "A-roas", "text": "Paid installs pay back within the D7 ROAS target", "owner": "ua", "module": "ua",
 "confidence": 0.52, "load_bearing": true, "created": 1, "last_confirmed": 30, "status": "active",
 "indicators": [{"expr": "roas_d7 >= 1.0", "effect": "confirm"}, {"expr": "roas_d7 < 0.8", "effect": "refute"}],
 "depends": [], "history": [{"day": 12, "confidence": 0.52, "reason": "refuted by roas_d7 < 0.8"}],
 "stale_since": null}
```

### 6.2 PublicCalibration (aggregates and AI only — §13.5)

```json
{"overall_ai": Profile, "overall_human": Profile,
 "by_module": {"ua": Profile}, "by_class": {"ua.bid_adjust": Profile}, "open_forecasts": 4}
```
**Profile**: `{"n": 25, "brier": 0.37, "mean_p": 0.63, "base_rate": 0.12,
"bins": [{"p_lo": 0.6, "p_hi": 0.8, "n": 22, "mean_p": 0.65, "freq": 0.14}]}`
(`brier`, `mean_p`, `base_rate` are `null` when `n` = 0; bins are 0.2 wide and only
non-empty bins are listed). `overall_human` aggregates all people; per-person profiles
are never public.

### 6.3 Private profile — `state/private/<id>.json`, `GET /calibration/person/:id` (admin token)

```json
{"person": "ceo", "visibility": "private: only this person may read it (§13.5)", "calibration": Profile}
```

### 6.4 QueueItem

```json
{"id": "Q-0100-0001", "person": "ceo", "kind": "approval", "created_day": 100, "decision": "D-0100-0002" | null,
 "subject": "Approval needed: ua.budget_shift: set_budget(daily=…)", "payload": {}, "blocking": true}
```
`payload` by kind: `autonomy_change` `{"change": "AU-…", "class": "…"}`;
`scenario_approval` `{"scenario": "SC-…"}`; `review` of an assumption
`{"assumption": "A-…", "text": "…", "last_confirmed": 12}`; `brief_approval`
`{"headline": "…"}`; `meta_loop_proposal` `{"finding": "ML-…"}`; `gate_review` `{"rule": "…"}`.

## 7. Logs (`log/*.jsonl`, one JSON object per line)

* `kpis.jsonl`: `{"day": 12, "date": "2026-01-16", "<kpi>": 3.15, …, "ota.decisions": 1,
  "ota.throughput": 1, "ota.human_minutes": 37.7, "ota.alerts_routed": 2, "ota.alerts_suppressed": 0,
  "ota.llm_cost_usd": 0.0, "ota.decision_latency_days": 0.0, "ota.loop_speed_days": 0.0,
  "ota.learning_rate": 1.07, "ota.hops_to_decider": 1.0, "ota.real_human_share": 0.0,
  "ota.external_signal_share": 0.5, "ota.ledger_completeness": 0.27, "ota.brier_30d": 0.34}`
  (`ota.*` optional metrics are omitted when null).
* `ledger.jsonl`: `{"day": 12, "id": "D-…", "class": "…", "module": "…", "status": "executing", "tier": "small", "what": "…", "success": null}`
  — one line per decision created or changed that day.
* `llm.jsonl`: `LlmCallRecord` + `"day"`.
* `human.jsonl`: `HumanAction`.

`GET /series?metrics=dau,ota.decisions&from=1&to=90` reads `kpis.jsonl`:

```json
{"days": [1, 2, 3], "dates": ["2026-01-05", "2026-01-06", "2026-01-07"],
 "series": {"dau": [43205.4, 43980.1, 44100.0], "ota.decisions": [0, 1, null]}}
```
Without `metrics`, every column is returned. Missing values are `null`.

## 8. Run control

`GET /run/status` (also `data/run_status.json`):

```json
{"running": true, "trigger": "api", "requested_days": 5, "done_days": 2, "start_day": 121, "current_day": 122,
 "started_at": "2026-11-19T10:00:00Z", "finished_at": null, "cost_usd": 0.0123, "llm_mode": "live",
 "errors": [], "last_autopilot_date": "2026-11-19"}
```
`trigger`: `api` | `autopilot`. `errors` holds the error of a failed run (the days
completed before the error are kept).

`POST /run` — header `X-OTA-Admin: <token>`, body `{"days": N}` (1 ≤ N ≤
`OTA_MAX_DAYS_PER_RUN`, default 31). → `200 {"started": true, "days": N}`; `401` wrong
token; `403` admin disabled (no `OTA_ADMIN_TOKEN`); `409` a run is in progress.

`POST /human-input` — header `X-OTA-Admin`, body `HumanInput`:

```json
{"person": "ceo", "item": "Q-0100-0001", "verdict": "reject", "reason": "We are not ready to scale"}
{"person": "ceo", "decision": "D-0100-0002", "verdict": "approve", "reason": "…"}
{"person": "ceo", "framing": "Focus on retention, not installs", "to": ["ua", "analytics"]}
```
→ `200 {"queued": true, "applies_on_day": 121, "file": "inbox/…json"}`. Inputs are
applied at the start of the next tick, before stand-ins, and recorded with
`source: "real_human"`. A verdict for an item resolves it; a verdict on a decision with
no open item is recorded as an `Override`. `framing` is sent as a `request` message with
`hops: 0` (default recipients: all modules).

## 9. Other endpoints

| Endpoint | Returns |
|---|---|
| `GET /health` | `ok` (text) |
| `GET /days` | `{"days": [1, 2, …], "range": {"first": 1, "last": 120, "count": 120}}` |

All endpoints are under the configured base path (default `/ota/api`). Errors are
`{"error": "message"}` with a 4xx/5xx status. GET responses carry permissive CORS
headers. The optional static viewer is served at `OTA_VIEWER_PATH` (default `/ota/sim`).
