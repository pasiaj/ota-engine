// Static knowledge about the engine (ARCHITECTURE.md, §13 amendments win) used for labels and docs.
// Concept links point at the book atlas, which lives one level up (/ota/vocabulary/#id).

export const ATLAS = '../';
export const vocab = id => `${ATLAS}vocabulary/#${id}`;
export const conceptLink = (id, label) => `<a class="concept" href="${vocab(id)}" target="_blank" rel="noopener">${label}</a>`;

export const PHASE_COL = { observe: 'var(--obs)', think: 'var(--think)', act: 'var(--act)' };
export const TIER_COL = { rule: '#5fd38d', small: '#8cc8ff', large: '#a98bff', human: '#f1d27a' };
export const TIERS = ['rule', 'small', 'large', 'human'];
export const RUNGS = ['inform', 'recommend', 'act_with_approval', 'act_then_review', 'autonomous'];
export const RUNG_SHORT = { inform: 'Inform', recommend: 'Recommend', act_with_approval: 'Act w/ approval', act_then_review: 'Act, then review', autonomous: 'Autonomous' };
export const KIND_COL = { leader: '#f1d27a', validator: '#ffb35c', end_node: '#e276d6', auditor: '#7fb2c9', ai_module: '#8cc8ff', toolkit: '#3fd0c9' };
export const GATE_COL = { green: 'var(--ok)', yellow: '#ffc94a', red: 'var(--danger)' };
export const STATUS_COL = { proposed: '#8cc8ff', approved: '#5fd38d', executing: '#ffc94a', sealed: '#a98bff', rejected: '#ff5d5d', reverted: '#ff8fb1', blocked: '#ff5d5d', deferred: '#a39780' };
export const MODALITIES = ['direction', 'structure', 'processes', 'capability', 'culture'];
export const PHASES = ['observe', 'think', 'act'];

// Engine tool id → book identity. ch = where the book treats it (ch 13 is the toolkit overview).
export const TOOLS = {
  ledger:      { name: 'Decision Ledger', glyph: 'decision-ledger', vocab: 'decision-ledger', phase: 'act', ch: '13, 16', from: 'Decision record',
                 does: 'Records every decision with the seven fields (what, when, who, information, assumptions, expected outcome, decision switches), seals the outcome on the check date and keeps the history of each entry.',
                 inputs: 'Proposed decisions from modules; approvals; switch evaluations.', outputs: '`Decision` records, `Outcome`s, `ledger_completeness`.' },
  assumptions: { name: 'Assumption Cycle', glyph: 'assumption-cycle', vocab: 'assumption-cycle', phase: 'think', ch: '13, 15', from: 'Assumption register / “what would have to be true”',
                 does: 'Evaluates each active assumption’s confirm/refute indicators daily, moves its confidence, ages it to stale after 60 days without signal and flags dependent decisions and OKRs. Stale assumptions nobody renews or archives form the graveyard.',
                 inputs: 'KPIs, indicator expressions from `assumptions.md`.', outputs: '`AssumptionUpdate`s, review queue items, graveyard.' },
  adversarial: { name: 'Adversarial Engine', glyph: 'adversarial-engine', vocab: 'adversarial-engine', phase: 'think', ch: '13, 15', from: 'Red team / designated challenger',
                 does: 'Attaches the strongest counter-argument to each material assumption a decision rests on. The deciding module must respond; a leader may override, and every override is logged with a reason.',
                 inputs: 'Proposed decisions and their load-bearing assumptions.', outputs: '`Challenge` records (strength, response, override).' },
  premortem:   { name: 'Automatic Pre-Mortem', glyph: 'automatic-pre-mortem', vocab: 'automatic-pre-mortem', phase: 'think', ch: '13, 15', from: 'Pre-mortem',
                 does: 'When stakes × novelty crosses the company threshold, imagines the decision has already failed and lists failure modes with probability and severity. Skips are recorded; the gap between pre-mortem start and commitment is watched (decision laundering).',
                 inputs: 'Decisions above `premortem_threshold`.', outputs: '`PreMortem` records, `premortems_run/skipped`.' },
  lattice:     { name: 'Reality Lattice', glyph: 'reality-lattice', vocab: 'reality-lattice', phase: 'observe', ch: '13, 14', from: 'KPI dashboards / observation stack',
                 does: 'Six levels: L1 coverage, L2 baselines (28-day, weekday-adjusted), L3 detection (z-scores, trend breaks, anomaly clusters), L4 interpretation (small LLM), L5 routing under a per-node alert budget, L6 the decider. Over-budget alerts queue and must be explicitly dismissed or acted on.',
                 inputs: 'Telemetry signals.', outputs: '`Deviation`s with level, routing and disposition.' },
  anomalies:   { name: 'Observation register', glyph: 'anomaly-diary', vocab: 'observation-register', phase: 'observe', ch: '14', from: 'Anomaly diary',
                 does: 'Humans and modules note things that feel off below any threshold. When three independent sources agree on a cluster, the Lattice promotes it to a deviation routed to leaders.',
                 inputs: 'Anomaly notes from end nodes and modules.', outputs: '`Anomaly` records, `anomaly_cluster` deviations.' },
  brief:       { name: 'Signal Brief', glyph: 'signal-brief', vocab: 'signal-brief', phase: 'observe', ch: '13, 14', from: 'Weekly report / CEO’s Monday email',
                 does: 'A daily one-screen brief for leadership drafted by the small LLM from the day’s deviations and events; a leader approves each brief.',
                 inputs: 'Deviations, events.', outputs: '`SignalBrief` (headline, items, approver).' },
  scenarios:   { name: 'Living Scenario Tree', glyph: 'living-scenario-tree', vocab: 'living-scenario-tree', phase: 'think', ch: '13, 15', from: 'Scenario mapping',
                 does: 'Keeps several futures alive as a DAG with probabilities and leading indicators. Major branches are approved by leadership monthly, minor ones are logged, urgent ones go to a named decider.',
                 inputs: 'Large deviations, anomaly clusters, indicator expressions.', outputs: '`ScenarioNode` changes.' },
  calibration: { name: 'Calibration Tracker', glyph: 'calibration-tracker', vocab: 'calibration-tracker', phase: 'act', ch: '13, 16', from: 'Forecasting pool',
                 does: 'Scores every forecast with the Brier score when it resolves and builds calibration profiles per module and decision class. Individual human profiles are private to that person; the viewer only shows aggregates.',
                 inputs: 'Forecasts attached to decisions; resolutions.', outputs: 'Profiles (n, Brier, bins), `brier_30d`.' },
  autonomy:    { name: 'Autonomy Ladder', glyph: 'autonomy-ladder', vocab: 'autonomy-ladder', phase: 'act', ch: '13, 16', from: 'Delegation (“under €5k, just do it”)',
                 does: 'Moves each decision class between rungs (inform → recommend → act with approval → act then review → autonomous) on Brier score plus agreement with human judgment. Path 2 reviewed monthly, Path 1 quarterly, demotions off-cycle; the accountable leader approves.',
                 inputs: 'Calibration per class, human agreement.', outputs: '`AutonomyChange`s, the rung matrix.' },
  gate:        { name: 'Domain Expert Gate', glyph: 'domain-expert-gate', vocab: 'domain-expert-gate', phase: 'act', ch: '13, 16', from: 'Domain expert sign-off',
                 does: 'Colours each decision by rules the validators wrote: green passes, yellow informs the validator and proceeds, red blocks until an independent expert review. With earned Path 2 authority the AI may review, except human-only classes.',
                 inputs: 'Decisions, validator `gate_rules`.', outputs: '`GateReview` on each decision.' },
  okrs:        { name: 'Living OKRs', glyph: 'direction-alignment', vocab: 'living-okrs', phase: 'think', ch: '13, 15', from: 'OKRs',
                 does: 'Tracks key results daily against targets and flags an OKR to its setter when it drifts or its assumptions weaken; the setter decides.',
                 inputs: 'KPIs, OKR assumptions.', outputs: '`OkrState` flags.' },
  coherence:   { name: 'Strategy Coherence Engine', glyph: 'strategy-coherence-engine', vocab: 'strategy-coherence-engine', phase: 'think', ch: '13, 15', from: 'Strategy review',
                 does: 'Checks the claims graph (mission, strategy, OKRs, budgets, incentives, policies, regulation) for gaps: spend that does not match strategy, unsupported OKRs, dangling links, regulation gaps.',
                 inputs: '`strategy.md` claims, ledger spend.', outputs: '`coherence` findings, `coherence_gap`.' },
  double_loop: { name: 'Double-loop learning', glyph: 'double-loop-learning', vocab: 'double-loop-learning', phase: 'act', ch: '13, 16', from: 'After-action review',
                 does: 'Weekly: looks for decision classes whose outcomes keep failing and asks whether the rule (not the bid) is wrong. Recurring findings feed Sentinel (learning-loop inversion).',
                 inputs: 'Sealed outcomes.', outputs: '`double_loop` findings.' },
  sentinel:    { name: 'Sentinel', glyph: 'sentinel', vocab: 'sentinel', phase: 'monitor', ch: '13, 16', from: '“What are we not seeing” memo',
                 does: 'Daily scan of the data estate for the six pathogens, the 13 AI failure modes, the four signal classes and rules (pilot limbo, big bang, missing action elements). A kind re-fires at most weekly.',
                 inputs: 'All traces.', outputs: '`SentinelFinding`s.' },
  meta_loop:   { name: 'Meta-loop', glyph: 'meta-loop', vocab: 'meta-loop', phase: 'monitor', ch: '13', from: '“How we work” retro',
                 does: 'Monthly: reads external research/peer practice and Sentinel’s own performance and proposes toolkit changes. It only proposes; leadership approves.',
                 inputs: 'Research feed, Sentinel stats.', outputs: '`meta_loop` findings and queue items.' },
  audit:       { name: 'Independent audit', glyph: 'instrument-audit', vocab: 'independent-audit', phase: 'monitor', ch: '13', from: 'Internal / external audit',
                 does: 'Monthly, on a separate read-only code path: audits Sentinel’s integrity, accountability coverage and the instruments; raises epistemic-corruption findings.',
                 inputs: 'Read-only store.', outputs: '`audit` findings, `epistemic_corruption` Sentinel findings.' },
  grid:        { name: 'Success grid', glyph: 'flywheel', vocab: 'ota-200', phase: 'monitor', ch: '6–12', from: 'Board self-assessment',
                 does: 'Quarterly self-assessment of 5 modalities × 3 phases, compared with the OTA 200 failure and success profiles; tags the universal anchor (think × direction) and the culture trap.',
                 inputs: 'Tool coverage and behaviour metrics.', outputs: '`GridScore`.' }
};
// Toolkit module ids (company MODULE.md with tool:) → engine tool id, plus pseudo-nodes.
export const NODE_TOOL = { 'reality-lattice': 'lattice', 'signal-brief': 'brief', 'decision-ledger': 'ledger', 'assumption-cycle': 'assumptions',
  'adversarial-engine': 'adversarial', premortem: 'premortem', 'scenario-tree': 'scenarios', 'strategy-coherence': 'coherence', 'living-okrs': 'okrs',
  'calibration-tracker': 'calibration', 'autonomy-ladder': 'autonomy', 'domain-expert-gate': 'gate', 'double-loop': 'double_loop', sentinel: 'sentinel',
  'meta-loop': 'meta_loop', 'independent-audit': 'audit' };

export const PATHOGENS = {
  hubris: 'Mean stated confidence rising while the Brier score is flat or worse.',
  complacency: 'Deviations ≥2σ with no decision within N days; switch checks overdue.',
  normalisation_of_deviance: 'Accepted exceptions or overrides of the same kind rising week over week.',
  groupthink: 'Leaders override adversarial challenges above a set rate; low diversity of pre-mortem failure modes.',
  misleading_incentives: 'Spend mix of ledger decisions diverging from strategy (coherence gap rising).',
  inertia: 'Stop conditions crossed without a stop decision; stale assumptions rising.'
};
export const FAILURE_MODES = {
  bypass: 'Human decisions taken outside the ledger (end-node actions not traced to a decision).',
  abdication: 'Rubber-stamping: approval time below a minimum on high-stakes items, or human reasons copying the LLM rationale.',
  synthetic_certainty: 'Single-source, high-confidence decisions without triangulation.',
  alert_fatigue: 'Alerts suppressed or ignored above budget; acknowledgement latency rising.',
  ai_echo_chamber: 'Share of external-provenance signals in inputs falling.',
  decision_laundering: 'Gap between pre-mortem start and commitment falling below a minimum: the pre-mortem as ritual.',
  assumption_graveyard: 'Stale assumptions piling up with no archive or renew decision.',
  glass_cockpit: 'Skills wasting away: falling share of manual cycles where a human reads raw data before the AI summary.',
  learning_loop_inversion: 'Double-loop recommendations keep recurring while external signals show a regime shift.',
  diffusion_of_accountability: 'A decision without an accountable human, or many approvers and no named owner.',
  homogenisation: 'Module outputs converging; dissent and input diversity falling.',
  epistemic_corruption: 'Calibration drifting in favour of the scored party (raised by the independent audit).',
  cultural_boundary: 'Practices that fail across cultural boundaries of the organisation.'
};
export const SIGNAL_CLASSES = {
  calibration_drift: 'Brier scores worsening over a trailing window.',
  capability_decay: 'Human skills or manual checks thinning out.',
  falling_dissent: 'Fewer challenges, fewer rejections, more unanimous approvals.',
  decaying_metrics: 'Metrics that no longer move with reality (stale or gamed instruments).'
};
export const SENTINEL_RULES = {
  pilot_limbo: 'More CPI tests open than `pilot_limbo_n` without a go/kill decision.',
  big_bang: 'A scale-up decision without a preceding test.',
  missing_action_elements: 'Prototype, launch or scale decisions missing hypothesis, learning metric, decision point or reversibility (ch 4).'
};
const VOCAB_FIX = { normalisation_of_deviance: 'normalisation-of-deviance', misleading_incentives: 'misleading-incentives', ai_echo_chamber: 'ai-echo-chamber',
  homogenisation: 'homogenisation-risk', pilot_limbo: 'pilot-limbo', big_bang: 'big-bang', missing_action_elements: 'four-missing-action-elements' };
export const kindVocab = name => VOCAB_FIX[name] || String(name).replace(/_/g, '-');
export const kindGlyph = (cat, name) => cat === 'pathogen' ? kindVocab(name) : null;

// The daily tick (§6), in order. `tool` = the TOOLS entry that runs it.
export const TICK = [
  { id: 'env', n: 'Environment', ph: 'world', text: 'The market model advances one day; the event generator writes 0–6 external events (storyline beats in history, LLM in live days). Events change market parameters and decay.', rec: 'events' },
  { id: 'estate', n: 'Data estate', ph: 'observe', text: 'Telemetry and events become attributable signals with provenance (internal / external).', rec: 'signals', tool: null },
  { id: 'lattice', n: 'Reality Lattice', ph: 'observe', text: 'L1 coverage, L2 baselines, L3 detection, L4 interpretation (small LLM), L5 routing under per-node alert budgets. Overflow is queued, never silently dropped.', rec: 'deviations', tool: 'lattice' },
  { id: 'assume', n: 'Assumption Cycle', ph: 'think', text: 'Indicators confirm or refute assumptions; stale ones age into the graveyard; OKRs drifting are flagged to their setter.', rec: 'assumption_updates', tool: 'assumptions' },
  { id: 'switch', n: 'Decision switches', ph: 'act', text: 'Ledger decisions whose check date is today are evaluated against confirm/stop expressions and sealed; forecasts are scored.', rec: 'decision_updates', tool: 'ledger' },
  { id: 'think', n: 'Module think', ph: 'think', text: 'Each AI module whose cadence fires reads its inbox and evaluates its decision tables: rules first; a row may escalate to the small LLM, the large LLM, or a human.', rec: 'module_runs', sub: ['rules', 'small LLM', 'large LLM', 'human'] },
  { id: 'gov', n: 'Governance', ph: 'think', text: 'Decision architecture looks up the class; pre-mortem if stakes × novelty is high; adversarial challenges; the expert gate colours it; the autonomy rung decides execute / wait / recommend; the ledger records the seven fields.', rec: 'decisions', sub: ['architecture', 'pre-mortem', 'adversarial', 'gate', 'autonomy', 'ledger'] },
  { id: 'human', n: 'Humans', ph: 'think', text: 'People (stand-ins, or real humans through the viewer) work their queues: approvals, gate reviews, framing, autonomy changes. Every minute is counted.', rec: 'human_actions' },
  { id: 'act', n: 'Act', ph: 'act', text: 'Approved actions are applied to the company state; their effect reaches the market tomorrow.', rec: 'actions' },
  { id: 'learn', n: 'Learn', ph: 'act', text: 'Calibration profiles, autonomy reviews, double-loop learning and strategy coherence on their cadences.', rec: 'autonomy_changes', tool: 'calibration' },
  { id: 'monitor', n: 'Monitor', ph: 'monitor', text: 'Sentinel scans for pathogens and AI failure modes; the Signal Brief is drafted; monthly meta-loop and independent audit; quarterly success grid.', rec: 'sentinel', tool: 'sentinel' }
];
