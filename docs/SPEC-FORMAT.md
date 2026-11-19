# Company spec format

A company built on the OTA engine is a directory of human-readable files. `ota-spec`
loads it with `ota_spec::load_company(dir)`. The reference example is
`examples/tiny/company/`.

```
<company>/
  company.md          # required: front-matter + description
  constitution.md     # numbered principles
  toolkit.md          # layered rollout: tool → enabled_from day
  llm.toml            # LLM tiers and daily budget
  kpis.md             # company KPIs (table)              optional
  assumptions.md      # company-level assumptions         optional
  okrs.md             # Living OKRs                       optional
  scenarios.md        # seed scenario tree                optional
  strategy.md         # Strategy Coherence claims         optional
  modules/**/<module>/
    MODULE.md         # required per module: front-matter
    decisions/*.md    # decision classes + decision tables
    prompts/*.md      # LLM prompts (think.md is used for module think)
    assumptions.md    # module assumptions                optional
    kpis.md           # metrics this module owns          optional
    playbook.md       # narrative guide                   optional
  people/<id>/
    PERSONA.md        # required per person: front-matter + persona text
    judgment.md       # how they judge (values, red lines, framing questions)
    memory.md         # optional; the engine may append notable decisions
```

## General rules

* **Leniency.** Keys are case-insensitive and `-`/space become `_` (`Load-Bearing` =
  `load_bearing`). Unknown keys are ignored (decision-class extras are kept in `extra`).
  Lists may be YAML lists, `[a, b]` or `a, b`. Strings may be quoted with `"`, `'` or
  backticks. A trailing ` # comment` on a bullet value is stripped.
* **Strictness.** Meaningful errors fail the load and **all** errors are reported
  together, each prefixed with the file path. Errors include: a module without an
  `accountable` human (constitution principle 6), an `accountable` that is not a
  person under `people/`, an expression that does not parse, an unknown enum value
  (the message lists valid values), a layered-rollout violation, duplicate ids.
  Non-fatal issues become `CompanySpec.warnings`.
* **Enum values** (snake_case; dashes/spaces also accepted):
  * reversibility: `reversible | costly_to_reverse | irreversible`
  * impact: `local | cross_function | company_wide`
  * rung: `inform | recommend | act_with_approval | act_then_review | autonomous`
  * path: `path1 | path2`
  * decision kind: `routine | test | go_kill | launch | scale`
  * phase: `observe | think | act`; modality: `direction | structure | processes | capability | culture`
  * cadence: `daily | weekly | monthly | quarterly`
  * role (people): `leader | validator | end_node | auditor`
  * scenario tier: `major | minor | urgent`
  * claim kind: `mission | strategy | okr | budget | incentive | policy | regulation`
  * escalate: empty, `small`, `large`, `human`

## Expressions

Used in decision tables (`when`, value parts of `then`), `confirm_if`, `stop_if`,
`forecast_if`, indicator rules, gate rules and OKR/scenario indicators.

* numbers (`1.15`, `1_000`, `15%` = 0.15), identifiers (`roas_d7`, `kc2.dau`; letters,
  digits, `_`, `.`, `:`), `+ - * /`, `< <= > >= == !=` (also `≥ ≤`), `&& || !` (also
  `and or not`), parentheses, `true`/`false`.
* functions: `min(a, b, …)`, `max(…)`, `abs(x)`, `clamp(x, lo, hi)`,
  `z(metric)` (z-score vs the 28-day baseline), `trend(metric[, days=7])` (relative slope
  per day), `days_since(name)` (days since the last decision of class `name`, the last
  action of kind `name`, or the last event of kind/tag `name`), `mean(metric[, days=28])`,
  `prev(metric[, days=1])`.
* Identifiers resolve to today's KPI values; in gate rules also to the decision's
  `stakes_eur`, `novelty`, `confidence`, `reversible` (1/0), `irreversible`,
  `company_wide`, `path2`.
* A reference to an unknown metric makes the expression *not match* and is recorded in
  the trace (`rules_failed`); it never matches by accident.
* Inside a Markdown table cell, write `||` inside backticks (`` `a || b` ``), as `or`,
  or escape it as `\|\|`. Pipes inside backticks do not split cells.

## company.md

```markdown
---
id: louhi                       # default: directory name
name: Louhi Games Oy            # default: first `# heading`
start_date: 2025-11-20          # calendar date of simulation day 1 (YYYY-MM-DD)
timezone: Europe/Helsinki
currency: EUR
premortem_threshold: 1000       # auto pre-mortem when stakes_eur × novelty ≥ this
alert_budget: 5                 # default per-node alerts/day (module may override)
learning_curve_kpis: [cpi_test_cost, prototype_cycle_days]   # Wright-curve learning rate
pilot_limbo_n: 3                # Sentinel: > N open tests without go/kill
---
# Louhi Games Oy
Free text description (shown in the viewer).
```

All front-matter keys are also exposed verbatim as `company.extra`.

## constitution.md

A numbered list. An optional `enforced by:` suffix names the enforcement point.

```markdown
1. Every decision is a hypothesis. — enforced by: ledger switches
2. With us, telling the truth is safe.
```

## toolkit.md

Either a table (preferred) or YAML front-matter `tools: {ledger: 1, lattice: {enabled_from: "month 4"}}`.

```markdown
| tool        | enabled_from | evolves_from          | human_checkpoint        |
|-------------|--------------|-----------------------|-------------------------|
| ledger      | 1            | decision record       | weekly ledger review    |
| assumptions | month 2      | assumption register   |                         |
| adversarial | day 61       | red team              |                         |
```

* `enabled_from`: `N`, `day N`, `DNNNN`, `week N` (= 7(N−1)+1), `month N` (= 30(N−1)+1),
  `quarter N` (= 91(N−1)+1).
* Tool ids (aliases accepted, e.g. `decision-ledger`, `reality lattice`,
  `auto pre-mortem`): `ledger, assumptions, adversarial, premortem, lattice, anomalies,
  brief, scenarios, calibration, autonomy, gate, okrs, coherence, double_loop, sentinel,
  meta_loop, audit, grid`.
* Fixed dependencies (a tool may not be enabled before them; violation = load error):

  | tool | depends on | | tool | depends on |
  |---|---|---|---|---|
  | assumptions | ledger | | autonomy | calibration |
  | adversarial | assumptions | | gate, coherence, double_loop, grid, calibration, premortem, lattice | ledger |
  | anomalies, brief | lattice | | okrs, scenarios | assumptions |
  | sentinel | lattice, calibration | | meta_loop, audit | sentinel |

* Unlisted tools are enabled from day 1 (with a warning). A missing file enables all.

## llm.toml

```toml
daily_budget_usd = 2.00
fallback = ["large", "small"]     # order to fall down when the budget is exhausted
cache = true                      # on-disk response cache keyed by request hash
max_retries = 3

[tiers.small]
provider = "openai"               # any OpenAI-compatible endpoint
model = "gpt-4.1-nano"
base_url = "https://api.openai.com/v1"   # default
api_key_env = "OPENAI_API_KEY"           # default
price_in = 0.10                   # USD per 1M input tokens
price_out = 0.40                  # USD per 1M output tokens
max_tokens = 2000                 # optional
temperature = 0.2                 # optional

[tiers.large]
model = "gpt-4.1-mini"
price_in = 0.40
price_out = 1.60

[tiers.env]     # environment event generator
model = "gpt-4.1-mini"
[tiers.human]   # default tier for stand-ins (PERSONA.md llm_tier overrides)
model = "gpt-4.1-mini"
```

Note: each key on its own line (TOML does not allow several `k = v` on one line).

## modules/**/MODULE.md

The module directory may be nested at any depth under `modules/` (e.g.
`modules/growth/ua-kc2/`).

```markdown
---
id: ua-kc2                    # default: directory name
name: UA — Kallio Climb 2     # default: first heading
org: growth                   # default: parent directory name
kind: ai_module               # ai_module | toolkit (default toolkit if `tool` is set)
tool: lattice                 # toolkit modules only: the engine tool this node represents
phases: [think, act]
cadence: daily                # daily | weekly | monthly | quarterly
inputs: [kc2.cpi, kc2.roas_d7, kc2.spend]   # metrics routed to / read by this module
outputs: [bids, budgets]
owner: ua-kc2
accountable: cmo              # REQUIRED: a person id under people/
evolves_from: UA manager      # the human predecessor
depends_on: [analytics]       # modules that think before this one
alert_budget: 4               # alerts/day this node can absorb (default company value)
modality: processes
metric_owner_of: [ua_cost]    # truth-pipeline signal domains this module owns
---
# UA — Kallio Climb 2
Mission, what good looks like, failure modes it watches for.
```

Module think order is a topological sort on `depends_on`, then observe-phase modules
before others, then id.

## decisions/*.md — decision classes

One file may hold several classes, each under a `## Decision class: <id>` heading.
Metadata are `- key: value` bullets before the table. (Alternatively, a file with YAML
front-matter `id:` and a single table.) If the heading has no id, the id is
`<module>.<file stem>`.

```markdown
## Decision class: ua.bid_adjust
- reversibility: reversible
- impact: local
- owner: ua-kc2                  # module that runs it (default: the directory's module)
- accountable: cmo               # default: the module's accountable
- path: path1                    # default: path1 iff reversible && local, else path2
- default_rung: act_then_review  # default: path1→act_then_review, irreversible→recommend, else act_with_approval
- stakes_eur: 500
- novelty: 0.2                   # 0..1; ≥0.3 escalates to small, >0.6 to large if no row escalates
- kind: routine                  # routine | test | go_kill | launch | scale
- modality: processes
- human_only: false              # irreversible classes the AI may never gate-review itself
- validator: ua-science          # Domain Expert Gate validator for this class
- assumptions: [A-roas]          # load-bearing assumptions → adversarial challenges
- forecast: "D7 ROAS ≥ target in 7 days"
- forecast_if: "roas_d7 >= 1.0"  # resolves the forecast (default: the switch outcome)
- forecast_horizon_days: 7
- confirm_if: "roas_d7 >= 1.0"   # decision switch: confirm signal
- stop_if: "roas_d7 < 0.7"       # decision switch: stop signal
- check_after_days: 7            # check_by = decision day + this
- hypothesis: "..."              # the four action elements (required for test/go_kill/launch/scale;
- learning_metric: roas_d7       #   the LLM may supply them for escalated decisions; missing ones are
- decision_point: "day-14 readout" #   recorded and counted by Sentinel)
- commitment_point: "..."        # scale decisions
- record_holds: false            # write ledger entries for `hold` outcomes too

| when                                       | then          | confidence | escalate | note |
|--------------------------------------------|---------------|-----------:|----------|------|
| `roas_d7 > 1.15 && spend_util > 0.9`       | bid *= 1.10   | 0.72       |          |      |
| `roas_d7 < 0.80`                           | bid *= 0.85   | 0.70       |          |      |
| `z(cpi) > 3`                               | hold          | 0.40       | small    |      |
| otherwise                                  | hold          | 0.60       |          |      |
```

* Rows are evaluated top to bottom; **the first matching row wins**. `otherwise`
  (also `else`, `default`, `*`) always matches. Columns `when` and `then` are required;
  `confidence` defaults to 0.5 (`72%` accepted); `escalate` is empty/`small`/`large`/`human`.
* `then` forms (all produce `ActionSpec {kind, params}` passed to `World::apply_action`):
  * `hold` / `none` / `-` / empty — no action (no ledger entry unless `record_holds`).
  * `target *= expr`, `+=`, `-=`, `/=`, `target = expr` →
    `{"kind":"adjust","params":{"target":"bid","op":"*","value":1.1}}` (`op` is `* + - / set`).
  * `kind(key=expr, key2="text", key3=bare_word)` → `{"kind":"kind","params":{...}}`;
    a single positional argument becomes `value`.
  * `kind` → `{"kind":"kind","params":{}}`.

## assumptions.md (module or company)

```markdown
## A-roas: Paid installs pay back within the D7 ROAS target
- confidence: 0.65
- load_bearing: true
- owner: ua-kc2          # default: module id (company file: first leader)
- confirm: roas_d7 >= 1.0     # any number of confirm/refute indicator rules
- refute: roas_d7 < 0.8
- depends: [ua.bid_adjust]    # decision classes resting on it (informational)
```

A table with columns `id | text | confidence | load_bearing | confirm | refute | owner`
is also accepted.

## kpis.md (module or company)

```markdown
| metric  | target | direction | unit | owner  | domain  | description      |
|---------|-------:|-----------|------|--------|---------|------------------|
| cpi     | 2.0    | down      | EUR  | ua     | ua_cost | cost per install |
```

`direction`: `up | down | band`. `owner` defaults to the module. `domain` marks a
strategic signal domain for the truth pipeline (`ua_cost`, `platform_policy`,
`competitors`, `player_sentiment`, ...).

## okrs.md

```markdown
## O1: Grow KC2 profitably
- setter: ceo                 # flags go to the setter (default: first leader)
- kr: kc2.dau >= 50000        # `metric >= | > | <= | < target`
- kr: kc2.roas_d7 >= 1.0
- assumptions: [A-roas]
```

## scenarios.md

```markdown
## S-cpi-shock: Platform change raises CPIs by 30 %
- parent: [S-root]            # DAG: any number of parents
- tier: major                 # major | minor | urgent
- p: 0.4
- confirm: z(cpi) > 2         # leading indicators
- refute: z(cpi) < 0
- assumptions: [A-roas]
- status: approved            # approved (default) | pending
```

## strategy.md — Strategy Coherence claims

```markdown
## C-budget-ua: Spend share on user acquisition
- kind: budget                # mission|strategy|okr|budget|incentive|policy|regulation
- links: [C-growth]
- org: growth                 # budget: share of ledger stakes by module org …
- spend_share: 0.7            # … expected (gap = |actual − expected|)

## C-att: ATT consent applies to UA changes
- kind: regulation
- applies_to: [ua.budget_shift, ua.*]
- requires: gate              # gate | premortem | human_approval
```

Every other bullet goes into `attrs`.

## people/<id>/PERSONA.md

```markdown
---
id: cmo                        # default: directory name
name: Aino Example
role: leader                   # leader | validator | end_node | auditor
title: CMO
accountable_for: [ua-kc2, ua.bid_adjust]   # modules or classes (informational)
expertise: [ua, brand]
llm_tier: human                # tier in llm.toml used for this stand-in
weekly_capacity_min: 900       # daily capacity = weekly / 5
shadow_group: false            # default true for end_node
bias:                          # also accepted as top-level keys
  overconfidence: 0.2
  challenge_acceptance: 0.7
  review_diligence: 0.8
gate_rules:                    # validators: the Domain Expert Gate classification
  - classes: ["ua.*"]          # class ids, `prefix.*` or "*"
    red_if: "stakes_eur >= 5000 && novelty > 0.5"
    yellow_if: "stakes_eur >= 1000"
---
# Aino Example
Persona text passed to the stand-in model.
```

`judgment.md` is free text (values, red lines, framing questions) and is passed to the
stand-in model alongside the persona and bias profile.
