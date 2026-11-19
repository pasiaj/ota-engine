// Docs: how the engine works inside. Real examples are reconstructed from /meta (the company's
// own MODULE.md, decision tables, people), so the docs stay true for whatever company is loaded.
import { esc, fmt, md, humanize, $ } from '../util.js';
import { TOOLS, TICK, PATHOGENS, FAILURE_MODES, SIGNAL_CLASSES, SENTINEL_RULES, RUNGS, RUNG_SHORT, TIER_COL, conceptLink, vocab, kindVocab, PHASE_COL } from '../catalog.js';
import { tableHTML } from './module.js';

const PAGES = [
  ['Start', [['overview', 'What the engine is'], ['tick', 'The daily tick & rollout'], ['views', 'Reading the views']]],
  ['Company spec', [['modules', 'Module Markdown format'], ['tables', 'Decision tables & expressions'], ['humans', 'Humans & stand-ins']]],
  ['Engine', [['llm', 'LLM tiers, budget, runs'], ['toolkit', 'Toolkit components'], ['governance', 'Ledger, gate, autonomy'], ['sentinel', 'Sentinel signatures'], ['data', 'Data model & API']]],
  ['Honesty', [['honest', 'Deviations from the book']]]
];
// help ids used across the viewer → page#anchor
const ALIAS = {
  'birdseye-scrubber': 'views#birdseye', 'birdseye-numbers': 'views#birdseye', 'birdseye-graph': 'views#graph',
  'module-strip': 'views#module', 'module-inputs': 'views#module', 'module-think': 'views#think', 'module-outputs': 'views#module',
  autonomy: 'governance#autonomy', 'ledger-fields': 'governance#ledger', decisions: 'governance#lifecycle', grid: 'toolkit#tool-grid', run: 'llm#run'
};

export async function render(el, ctx, rt) {
  let topic = rt.parts[0] || 'overview', anchor = null;
  if (ALIAS[topic]) [topic, anchor] = ALIAS[topic].split('#');
  if (topic.startsWith('tool-')) { anchor = topic; topic = 'toolkit'; }
  const page = BUILD[topic] ? topic : 'overview';
  el.innerHTML = `<div class="wrap docs"><nav aria-label="Docs">${PAGES.map(([g, l]) => `<div class="g">${g}</div>` + l.map(([id, n]) => `<a href="#/docs/${id}" class="${id === page ? 'on' : ''}">${n}</a>`).join('')).join('')}</nav>
    <article id="doc">${BUILD[page](ctx)}</article></div>`;
  if (anchor) setTimeout(() => { const a = document.getElementById(anchor); if (a) { a.scrollIntoView({ block: 'start' }); a.classList.add('tour-hl'); setTimeout(() => a.classList.remove('tour-hl'), 1800); } }, 30);
  el.querySelectorAll('#doc [data-goto]').forEach(a => a.onclick = e => { e.preventDefault(); document.getElementById(a.dataset.goto)?.scrollIntoView({ behavior: 'smooth' }); });
}

const kicker = t => `<div class="kicker">Docs · ${t}</div>`;
const BUILD = {
  overview: ctx => `${kicker('start')}<h1>What the engine is</h1>
    <p><b>ota-engine</b> is a company-agnostic Rust implementation of the ideas in Risto Siilasmaa’s <i>OTA — Havaitse, Ajattele, Toimi</i>. A company is described in plain Markdown — its modules, decision tables, people, strategy and toolkit rollout — and plugs in a market model. The engine then runs the company one simulated day at a time. This viewer reads the engine’s JSON through its HTTP API and never changes anything, except through the admin-protected <a href="#/run">Run</a> page.</p>
    <h3>Design principles, from the book</h3>
    <ol><li><b>The loop is the unit.</b> Every module runs ${conceptLink('observe', 'Observe')} → ${conceptLink('think', 'Think')} → ${conceptLink('act', 'Act')}, and the company is one loop with a ${conceptLink('meta-loop', 'meta-loop')} over the loops.</li>
    <li><b>Three muscles fail separately.</b> Each module’s day is stored as separate inputs, think trace and outputs.</li>
    <li><b>AI amplifies; it does not correct.</b> Each tool names the human practice it evolves from and keeps a human checkpoint.</li>
    <li><b>Decision rights are explicit</b> (${conceptLink('decision-architecture', 'decision architecture')}): every decision belongs to a class with reversibility, impact, an accountable human and an autonomy rung.</li>
    <li><b>Authority is earned per class</b> on calibration evidence (${conceptLink('autonomy-ladder', 'Autonomy Ladder')}).</li>
    <li><b>Every decision is a hypothesis with an exit</b> (${conceptLink('decision-switches', 'decision switches')}, ${conceptLink('outcome-sealing', 'outcome sealing')}).</li>
    <li><b>Humans do framing, judgment and accountability.</b> A named human is accountable at the top of every chain; the engine refuses a module without one.</li>
    <li><b>The system is a colleague, not an authority:</b> every recommendation carries ${conceptLink('human-legible-reasoning', 'human-legible reasoning')} and confidence; overrides are logged.</li>
    <li><b>${conceptLink('layered-rollout', 'Layered rollout')}:</b> a tool cannot switch on before the tools it depends on.</li>
    <li><b>${conceptLink('independent-audit', 'Independent audit')}:</b> Sentinel watches the toolkit; a separate read-only audit watches Sentinel.</li></ol>
    <h3>Layers</h3>
    <pre class="code">ENVIRONMENT   market model + event generator (the company's World)
DATA ESTATE   append-only store: signals, messages, decisions, assumptions, forecasts, outcomes
OBSERVE       Reality Lattice L1–L6 · Signal Brief · observation register
THINK         module decision tables → LLM tiers · Assumption Cycle · Adversarial Engine
              · Automatic Pre-Mortem · Scenario Tree · Strategy Coherence · Living OKRs
GOVERN        decision architecture · Autonomy Ladder · Domain Expert Gate · human stand-ins
ACT           action executor · Decision Ledger (7 fields + switches)
LEARN         outcome sealing · Calibration Tracker · double-loop learning
MONITOR       Sentinel · Meta-loop · Independent audit · success grid</pre>
    <h3>This company</h3><p>${esc(ctx.meta.company?.name)}: ${(ctx.meta.modules || []).length} modules, ${(ctx.meta.people || []).length} people, ${(ctx.meta.classes || []).length} decision classes, ${(ctx.meta.kpis || []).length} KPIs, engine ${esc(ctx.meta.engine_version)}.</p>`,

  tick: ctx => {
    const tk = [...(ctx.meta.toolkit || [])].sort((a, b) => a.enabled_from - b.enabled_from);
    const last = Math.max(ctx.days.last || 1, ...tk.map(t => t.enabled_from));
    return `${kicker('start')}<h1>The daily tick</h1><p>One simulated day runs these steps in order; each one is persisted in the day file (<code>GET /day/:d</code>). The field in brackets is where you find its output.</p>
    <div class="tscroll"><table class="t"><thead><tr><th>#</th><th>step</th><th>phase</th><th>what happens</th><th>record</th></tr></thead><tbody>${TICK.map((s, i) => `<tr><td>${i + 1}</td><td><b>${esc(s.n)}</b>${s.tool ? `<br><a class="tiny" href="#/docs/tool-${s.tool}">${esc(TOOLS[s.tool].name)}</a>` : ''}</td><td style="color:${PHASE_COL[s.ph] || 'var(--muted)'}">${s.ph}</td><td class="small">${esc(s.text)}</td><td class="mono tiny">${esc(s.rec || '')}</td></tr>`).join('')}</tbody></table></div>
    <h3>Cadences</h3><p>Weekly steps run on Mondays (calibration profiles, double-loop learning, coherence, off-cycle autonomy demotions), monthly ones on the 1st (meta-loop, independent audit, scenario approvals, Path 2 autonomy reviews), quarterly ones on Jan/Apr/Jul/Oct 1st (success grid, Path 1 autonomy reviews, a leader think week). Each day lists its <code>cadences</code>.</p>
    <h3 id="rollout">Layered rollout</h3><p>The book insists that the human versions of the tools come first and the AI tools are layered in order. Each tool has an <code>enabled_from</code> day; before it, its step is skipped and recorded as such (for example “auto pre-mortem not enabled yet”).</p>
    <svg viewBox="0 0 900 ${tk.length * 20 + 20}" width="100%" style="min-width:520px">${tk.map((t, i) => { const x = 170 + (t.enabled_from - 1) / Math.max(1, last - 1) * 700; return `<a href="#/docs/tool-${t.id}"><text x="160" y="${i * 20 + 14}" text-anchor="end" font-size="11" fill="var(--text)">${esc(TOOLS[t.id]?.name || t.id)}</text></a><rect x="${x}" y="${i * 20 + 4}" width="${870 - x}" height="12" rx="2" fill="${PHASE_COL[TOOLS[t.id]?.phase] || '#7fb2c9'}" fill-opacity=".45"/><text x="${x + 4}" y="${i * 20 + 14}" font-size="10" fill="var(--text)">day ${t.enabled_from}</text>`; }).join('')}</svg>`;
  },

  views: ctx => `${kicker('start')}<h1>Reading the views</h1>
    <h3 id="birdseye">Birdseye</h3><p>The scrubber selects a day (drag, click a sparkline, or press ← →). Sparklines show the whole run with a cursor at the selected day. Marks above them: <b style="color:#f1d27a">gold</b> storyline or live events, <b style="color:var(--danger)">red</b> Sentinel findings, <b style="color:#a98bff">violet</b> autonomy changes. The marks and the decisions-by-tier bars are filled by a background index that reads each day file once per session, so they appear progressively.</p>
    <p>Big numbers summarise the day: revenue and DAU (with change vs a week earlier), decisions committed by tier, module runs and LLM cost, alerts routed and over budget, human minutes, the 30-day real-human share and new Sentinel findings.</p>
    <h3 id="graph">The organisation graph</h3><ul><li><b>Rows</b>: leaders; validators (diamonds — the gate); AI modules in columns by organisation and rows by their first OTA phase (observe, think, act); the toolkit layer (squares; dashed when not yet enabled); end nodes at the rim.</li>
    <li><b>Edges</b> are the day’s messages, width ∝ √count, colour by intent: blue inform/report, gold propose/request, orange escalate/challenge. The busiest edges pulse.</li>
    <li><b>Ring</b>: green calm; yellow when deviations were routed, rules failed or a queue is long; red when alerts went over budget or a red gate blocked.</li>
    <li><b>Badges</b>: one dot per tier used that day — ${['rule', 'small', 'large', 'human'].map(t => `<span class="tier t-${t}">${t}</span>`).join(' ')}.</li><li>Node size grows with message volume. Click a node to open it.</li></ul>
    <h3 id="module">Module focus</h3><p>Three columns, the book’s three muscles kept apart: <b style="color:var(--obs)">inputs</b> (routed deviations, over-budget alerts it must dismiss or act on, inbox, decision switches due, assumption and OKR flags, the metric snapshot), <b style="color:var(--think)">think</b>, and <b style="color:var(--act)">outputs</b> (decisions, decision updates, executed actions, messages, forecasts, dismissed alerts, anomalies). The 90-day strip shows activity per day coloured by the highest tier used; height is decisions + messages.</p>
    <h3 id="think">The think trace</h3><p>Rules come first: each fired row shows the class, row number, its <code>when</code> and <code>then</code>, its confidence, any <code>escalate</code>, and the <b>bindings</b> — the values each identifier had when evaluated. If a row escalated, the single batched LLM call for the module is shown with its prompt and JSON response, model and cost; a <i>fallback</i> chip means the budget forced a cheaper tier. The rationale is the human-legible summary.</p>
    <h3>Decision, Health, People, Run</h3><p>See <a href="#/docs/governance">Ledger, gate, autonomy</a>, <a href="#/docs/sentinel">Sentinel</a>, <a href="#/docs/humans">Humans</a> and <a href="#/docs/llm">LLM tiers & runs</a>.</p>`,

  modules: ctx => {
    const mods = ctx.meta.modules || [];
    const ex = mods.find(m => !m.tool && (m.classes || []).length >= 2) || mods[0];
    const cls = ex ? (ctx.meta.classes || []).filter(c => c.module === ex.id) : [];
    const fm = ex ? ['id', 'name', 'org', 'kind', 'phases', 'cadence', 'inputs', 'outputs', 'owner', 'accountable', 'evolves_from', 'depends_on', 'alert_budget', 'modality', 'metric_owner_of'].filter(k => ex[k] != null && !(Array.isArray(ex[k]) && !ex[k].length)).map(k => `${k}: ${Array.isArray(ex[k]) ? '[' + ex[k].join(', ') + ']' : ex[k]}`).join('\n') : '';
    const c0 = cls[0];
    return `${kicker('company spec')}<h1>Module Markdown format</h1>
    <p>A company is a directory of Markdown files a person can read without the program. Each AI module is a directory:</p>
    <pre class="code">modules/&lt;org&gt;/&lt;module&gt;/
  MODULE.md            front-matter + mission, what good looks like, failure modes it watches
  decisions/*.md       one decision class per file: metadata + decision table
  prompts/think.md     system prompt for the LLM tier ({{inbox}} {{kpis}} {{assumptions}})
  assumptions.md       seed assumptions with confirm/refute indicator rules
  kpis.md              metrics it owns, with targets
  playbook.md          optional operating guide for humans</pre>
    <p>The loader refuses a module whose <code>MODULE.md</code> does not name an <code>accountable</code> human — constitution principle 6.</p>
    ${ex ? `<h3>A real example: <a href="${ctx.link.module(ex.id)}">${esc(ex.name)}</a></h3><p class="small muted">Reconstructed from <code>GET /meta</code> for the loaded company.</p>
    <pre class="code">---\n${esc(fm)}\n---</pre>${ex.description ? `<details open><summary>MODULE.md body</summary><div class="panel small" style="margin-top:6px">${md(ex.description)}</div></details>` : ''}
    ${c0 ? `<h3>One of its decision classes</h3><pre class="code">## Decision class: ${esc(c0.id)}
- reversibility: ${esc(c0.reversibility)}
- impact: ${esc(c0.impact)}
- path: ${esc(c0.path)}
- accountable: ${esc(c0.accountable)}
- default_rung: ${esc(c0.default_rung)}
- stakes_eur: ${c0.stakes_eur}
- novelty: ${c0.novelty}
- kind: ${esc(c0.kind)}${c0.human_only ? '\n- human_only: true' : ''}
- assumptions: [${(c0.assumptions || []).map(esc).join(', ')}]
- forecast: "${esc(c0.forecast || '')}"
- confirm_if: ${esc(c0.confirm_if || '')}
- stop_if: ${esc(c0.stop_if || '')}
- check_after_days: ${c0.check_after_days ?? ''}</pre>${tableHTML(c0)}${c0.description ? `<div class="small">${md(c0.description)}</div>` : ''}` : ''}` : ''}
    <h3>People</h3><p><code>people/&lt;id&gt;/PERSONA.md</code> (role, accountable classes, expertise, weekly capacity, LLM tier, bias profile, gate rules for validators) and <code>judgment.md</code> (values, red lines, framing questions). See <a href="#/docs/humans">Humans</a>.</p>
    <h3>Company files</h3><p><code>company.md</code> (profile, thresholds such as <code>premortem_threshold = ${fmt(ctx.meta.company?.premortem_threshold)}</code>, <code>alert_budget = ${fmt(ctx.meta.company?.alert_budget)}</code>), <code>constitution.md</code>, <code>strategy.md</code> (the claims graph), <code>okrs.md</code>, <code>scenarios.md</code>, <code>toolkit.md</code> (rollout days) and <code>llm.toml</code>.</p>
    ${(ctx.meta.constitution || []).length ? `<h3>The constitution and where each principle is enforced</h3><ol>${ctx.meta.constitution.map(c => `<li>${esc(c.text)} <span class="tiny faint">— ${esc(c.enforced_by || '')}</span></li>`).join('')}</ol>` : ''}`;
  },

  tables: ctx => `${kicker('company spec')}<h1>Decision tables & the expression language</h1>
    <p>Most decisions in an operating company are routine and reversible. They need a transparent, auditable rule, not a model. So a module thinks in <b>decision tables</b> first: rows are evaluated top to bottom, the <b>first match wins</b>, <code>otherwise</code> is the fallback. A row’s <code>then</code> is one action the company’s World executes (<code>bid *= 0.85</code>, <code>set_budget(daily=4000)</code>, <code>hold</code>).</p>
    <div class="tscroll"><table class="t mono small"><thead><tr><th>column</th><th>meaning</th></tr></thead><tbody>
    <tr><td>when</td><td>boolean expression over metrics</td></tr><tr><td>then</td><td>the proposed action</td></tr><tr><td>confidence</td><td>the forecast probability the decision carries (0–1); calibrated later</td></tr>
    <tr><td>escalate</td><td>empty, or <span class="tier t-small">small</span> <span class="tier t-large">large</span> <span class="tier t-human">human</span>: hand the case up a tier</td></tr><tr><td>note</td><td>follow-ups a human should know about</td></tr></tbody></table></div>
    <h3>When does a model get involved?</h3><p>Only when a row says <code>escalate</code>, no row fires, or novelty is high. All escalated items for one module go into <b>one</b> structured call a day (JSON schema in, array of decisions out). Responses are cached by request hash, so replays cost nothing.</p>
    <h3 id="expr">Expression language</h3><p>Small and deterministic, evaluated in <code>ota-decide</code>:</p>
    <div class="tscroll"><table class="t small"><tbody>
    <tr><td class="mono">numbers, metric ids</td><td><code>kc2_roas_d7</code>, <code>1.15</code></td></tr>
    <tr><td class="mono">+ - * /  &lt; &lt;= &gt; &gt;= == !=  &amp;&amp; || !</td><td>arithmetic, comparison, logic</td></tr>
    <tr><td class="mono">z(m)</td><td>z-score of today’s value vs the 28-day weekday-adjusted baseline</td></tr>
    <tr><td class="mono">trend(m)</td><td>recent slope</td></tr><tr><td class="mono">days_since(x)</td><td>days since an event or decision</td></tr>
    <tr><td class="mono">mean(m, n), prev(m)</td><td>rolling mean, yesterday’s value</td></tr><tr><td class="mono">min max abs clamp</td><td>the usual</td></tr></tbody></table></div>
    <p>The think trace shows the <b>bindings</b>: every identifier with the value it had when the row was evaluated, so any decision can be re-checked by hand.</p>
    <h3>All decision tables of this company</h3><p class="small muted">${(ctx.meta.classes || []).length} classes. Open one module’s tables from its <a href="#/module">module page</a>.</p>
    <details><summary>Show all</summary>${(ctx.meta.classes || []).map(c => tableHTML(c)).join('')}</details>`,

  humans: ctx => `${kicker('company spec')}<h1>Humans & stand-ins</h1>
    <p>The book gives humans three tasks that do not delegate: <b>framing</b> (what problem, which goals), <b>judgment</b> (is this right, given what we value) and <b>accountability</b> (my name is on it) — ${conceptLink('agency', 'agency')} in ch 17. Roles:</p>
    <ul><li><b>Leaders</b> frame goals, approve major scenario branches, decide irreversible classes and are accountable for domains. They read the Signal Brief.</li>
    <li><b>Validators</b> are the ${conceptLink('domain-expert-gate', 'Domain Expert Gate')}: they write the gate rules and review yellow/red claims. They never manage.</li>
    <li><b>End nodes</b> do what AI should not or cannot do alone — taste, play-feel, community voice, legal signature — own some Path 1 classes, write anomalies, raise challenges, and form the shadow group that reports straight to leadership.</li></ul>
    <h3 id="standins">Stand-ins</h3><p>Each person is played by an LLM persona (<code>PERSONA.md</code> + <code>judgment.md</code>) called with a strict schema: <code>{decision: approve|reject|amend|defer, reason, amendments, minutes_spent, confidence}</code>. A bias profile — <code>overconfidence</code>, <code>challenge_acceptance</code>, <code>review_diligence</code> — goes to the model and to the scripted responder, so pathogens can genuinely emerge and Sentinel has something real to find. Weekly capacity is enforced; queues build up when humans are the bottleneck.</p>
    <h3 id="real">Real humans</h3><p>Through <a href="#/run">Run → human input</a> a real person can send a framing message (hops = 0, to all or chosen modules), resolve a queue item or override a decision. The input is applied at the start of the next tick, before the stand-ins act, and every record carries <span class="src-real">source: real_human</span>. The <b>real-human share</b> (30 days) is shown on Birdseye, Health and People.</p>
    <h3 id="privacy">Privacy</h3><p>Individual human calibration (how well each person’s approvals predicted outcomes) is private to that person (§13.5): it is served only with the admin token and never shown in this viewer. Health shows aggregates and AI profiles only.</p>
    <h3>The people of ${esc(ctx.meta.company?.name || 'this company')}</h3><div class="tscroll"><table class="t small"><thead><tr><th>person</th><th>role</th><th>capacity / week</th><th>bias</th></tr></thead><tbody>${(ctx.meta.people || []).map(p => `<tr><td><a href="${ctx.link.person(p.id)}">${esc(p.name)}</a><div class="tiny faint">${esc(p.title || '')}</div></td><td>${esc(humanize(p.role))}</td><td>${fmt(p.weekly_capacity_min)} min</td><td class="tiny">${Object.entries(p.bias || {}).map(([k, v]) => `${esc(humanize(k))} ${fmt(v, 2)}`).join(' · ')}</td></tr>`).join('')}</tbody></table></div>`,

  llm: ctx => {
    const L = ctx.meta.llm || {};
    return `${kicker('engine')}<h1>LLM tiers, budget and runs</h1>
    <p>Models are configured as <b>tiers</b> in <code>llm.toml</code>; any OpenAI-compatible endpoint can be a tier. Decisions use the cheapest tier that can do the job:</p>
    <div class="tscroll"><table class="t small"><thead><tr><th>tier</th><th>used for</th><th>model here</th></tr></thead><tbody>
    <tr><td><span class="tier t-rule">rule</span></td><td>decision tables — most decisions, no model at all</td><td>–</td></tr>
    <tr><td><span class="tier t-small">small</span></td><td>lattice interpretation (L4), briefs, drafting, rows that escalate to small, moderate novelty</td><td class="mono">${esc(L.tiers?.small?.model || '–')}</td></tr>
    <tr><td><span class="tier t-large">large</span></td><td>high-stakes or novel decisions, pre-mortems, adversarial challenges, meta-loop proposals</td><td class="mono">${esc(L.tiers?.large?.model || '–')}</td></tr>
    <tr><td><span class="tier t-human">human</span></td><td>stand-in personas for approvals, gate reviews, framing</td><td class="mono">${esc(L.tiers?.human?.model || '–')}</td></tr>
    <tr><td><span class="tier" style="background:#7fb2c9">env</span></td><td>the event generator on live days</td><td class="mono">${esc(L.tiers?.env?.model || '–')}</td></tr></tbody></table></div>
    <h3 id="budget">Budget and fallback</h3><p>The daily cap is <b>$${fmt(L.daily_budget_usd)}</b>. When it is reached, calls fall down the tiers (${(L.fallback || []).map(esc).join(' → ')} → rules) or defer to tomorrow, and the fallback is written on the call (<code>fallback: "large (budget) → small"</code>) and shown in the think trace. Cost pressure is visible, never hidden. Every call is logged with tokens and cost (<code>llm_calls</code> in each day; the Run page sums cost per tier).</p>
    <h3 id="scripted">Scripted vs live</h3><p>History days use the company’s <b>scripted responder</b>: deterministic templates returning the same JSON schemas. Live days call real models when the server has an API key; if a live call fails, the scripted responder answers and the error is recorded. Each day carries its <code>llm_mode</code>. This data: <b>${esc(ctx.meta.llm_mode)}</b>${ctx.meta.scripted_until ? `, scripted until day ${ctx.meta.scripted_until}` : ''}.</p>
    <h3 id="run">Running days</h3><p><code>POST /run {days: N}</code> with the <code>X-OTA-Admin</code> header advances the company in the background; <code>GET /run/status</code> reports progress and cost. With autopilot on, the server advances one day per real day at a set hour. Day files are written atomically at the end of each tick; ledger and state snapshots at checkpoints (every 30 days and at the end of a run).</p>`;
  },

  toolkit: ctx => {
    const en = new Map((ctx.meta.toolkit || []).map(t => [t.id, t]));
    const ids = [...new Set([...Object.keys(TOOLS), ...en.keys()])];
    return `${kicker('engine')}<h1>Toolkit components</h1><p>The book’s Part IV toolkit as stateful services. Each names the human practice it evolves from (the book: the human version must come first) and a human checkpoint. Names link to the <a href="../vocabulary/">book atlas</a>.</p>
    <div class="row" style="margin-bottom:8px">${ids.map(id => `<a class="chip" href="#" data-goto="tool-${id}">${esc(TOOLS[id]?.name || id)}</a>`).join('')}</div>
    ${ids.map(id => { const t = TOOLS[id] || { name: id, does: '', glyph: null }; const e = en.get(id);
      return `<div class="toolcard" id="tool-${id}"><div>${t.glyph && window.OTA_ID ? window.OTA_ID.badge(t.glyph, 48) : ''}</div><div><h3 style="margin:0">${esc(t.name)} <span class="tiny faint mono">${esc(id)}</span></h3>
      <p class="tiny muted" style="margin:2px 0 6px">${t.phase ? `<b style="color:${PHASE_COL[t.phase] || 'var(--muted)'}">${t.phase}</b> · ` : ''}book ch ${esc(t.ch || '13')} · evolves from <i>${esc(e?.evolves_from && e.evolves_from !== '-' ? e.evolves_from : t.from || '–')}</i>${e ? ` · <b>enabled from day ${e.enabled_from}</b>${e.depends_on?.length ? ' · after ' + e.depends_on.map(d => `<a href="#" data-goto="tool-${d}">${esc(TOOLS[d]?.name || d)}</a>`).join(', ') : ''}` : ' · not configured for this company'}${t.vocab ? ' · ' + conceptLink(t.vocab, 'atlas ↗') : ''}</p>
      <p class="small" style="margin:0 0 4px">${esc(t.does)}</p>${t.inputs ? `<dl class="kv tiny"><dt>inputs</dt><dd>${md(t.inputs).replace(/^<p>|<\/p>$/g, '')}</dd><dt>outputs</dt><dd>${md(t.outputs).replace(/^<p>|<\/p>$/g, '')}</dd>${e?.human_checkpoint ? `<dt>human checkpoint</dt><dd>${esc(e.human_checkpoint)}</dd>` : ''}</dl>` : ''}</div></div>`; }).join('')}`;
  },

  governance: ctx => `${kicker('engine')}<h1>Ledger, gate, autonomy</h1>
    <h3 id="ledger">The seven fields</h3><p>The ${conceptLink('decision-ledger', 'Decision Ledger')} records every decision with: <b>① what</b>, <b>② when</b>, <b>③ who decided</b> (a module, or the approving person), <b>④ information</b> (the deviations, signals and decisions it cites), <b>⑤ assumptions</b>, <b>⑥ expected outcome</b> and <b>⑦ decision switches</b> — a confirm expression, a stop expression and a check-by day. On the check day the switches are evaluated and the outcome is <b>sealed</b>: confirmed, stopped, or inconclusive. A stop signal creates a <i>revert</i> decision. <code>ledger_completeness</code> measures how often all seven are filled.</p>
    <h3 id="lifecycle">Lifecycle and governance</h3><p><code>proposed</code> → (<code>blocked</code> at a red gate) → <code>approved</code> → <code>executing</code> → <code>sealed</code>; side exits <code>rejected</code>, <code>reverted</code>, <code>deferred</code>. Before a decision is committed: decision architecture looks up its class; an ${conceptLink('automatic-pre-mortem', 'automatic pre-mortem')} runs when stakes × novelty ≥ ${fmt(ctx.meta.company?.premortem_threshold)}; the ${conceptLink('adversarial-engine', 'Adversarial Engine')} attaches the strongest challenge per material assumption; the gate colours it; the rung decides whether it executes now, waits for approval or only recommends; impact decides who is informed. Prototype, launch and scale decisions must carry the ${conceptLink('four-missing-action-elements', 'four elements of action')}: hypothesis, learning metric, decision point, reversibility.</p>
    <h3 id="gate">Domain Expert Gate</h3><p>Validators write rules (<code>red_if</code>, <code>yellow_if</code> per class pattern); the system applies them. <b style="color:var(--ok)">Green</b> passes. <b style="color:#ffc94a">Yellow</b> informs the validator and proceeds. <b style="color:var(--danger)">Red</b> blocks until an independent expert review is recorded. Where the AI has earned Path 2 authority it may review itself (<code>reviewed_by: "ai"</code>), never for <code>human_only</code> classes.</p>
    <h3 id="autonomy">Autonomy Ladder</h3><p>Five rungs per class: ${RUNGS.map(r => `<b>${RUNG_SHORT[r]}</b>`).join(' → ')}. <b>Path 1</b> classes (reversible, local) start at <i>act then review</i> or higher; humans review patterns afterwards. <b>Path 2</b> classes must earn authority in a narrow domain. Evidence = the class’s Brier score plus its agreement rate with human judgments. Path 2 is reviewed monthly, Path 1 quarterly, and demotions happen off-cycle when thresholds are crossed. The accountable leader approves every change; every change is logged and reversible.</p>
    <h3 id="calibration">Calibration</h3><p>Each decision carries a forecast (its confidence that the confirm switch will fire). When it resolves, the Brier score (p − outcome)² is recorded: 0 is perfect, 0.25 is a coin flip at p = 0.5. Profiles per module and class drive the ladder.</p>`,

  sentinel: () => {
    const row = (cat, n, d) => `<tr><td><a href="${vocab(kindVocab(n))}" target="_blank" rel="noopener">${esc(humanize(n))}</a></td><td class="tiny faint">${cat}</td><td class="small">${esc(d)}</td></tr>`;
    return `${kicker('engine')}<h1>Sentinel signatures</h1><p>${conceptLink('sentinel', 'Sentinel')} scans the data estate every day for computable signatures over a trailing window. A kind re-fires at most once every 7 days. The ${conceptLink('independent-audit', 'independent audit')} runs monthly on a separate read-only path and raises <i>epistemic corruption</i>.</p>
    <h3>The six pathogens</h3><div class="tscroll"><table class="t">${Object.entries(PATHOGENS).map(([n, d]) => row('pathogen', n, d)).join('')}</table></div>
    <h3>The 13 AI failure modes</h3><div class="tscroll"><table class="t">${Object.entries(FAILURE_MODES).map(([n, d]) => row('failure mode', n, d)).join('')}</table></div>
    <h3>Signal classes</h3><div class="tscroll"><table class="t">${Object.entries(SIGNAL_CLASSES).map(([n, d]) => row('signal class', n, d)).join('')}</table></div>
    <h3>Rules</h3><div class="tscroll"><table class="t">${Object.entries(SENTINEL_RULES).map(([n, d]) => row('rule', n, d)).join('')}</table></div>
    <p class="small muted">Thresholds are engine constants today (not yet configurable per company). See them fire on <a href="#/health">Health</a>.</p>`;
  },

  data: () => `${kicker('engine')}<h1>Data model & API</h1><p>Everything the engine writes is plain JSON; the API serves the same shapes. Full contract: <code>docs/DATA.md</code> in the engine repository.</p>
    <div class="tscroll"><table class="t small"><thead><tr><th>endpoint</th><th>returns</th></tr></thead><tbody>
    <tr><td class="mono">GET /meta</td><td>company, constitution, toolkit, modules, people, decision classes, KPIs, LLM tiers (no keys), day range, run state</td></tr>
    <tr><td class="mono">GET /days</td><td>available days</td></tr><tr><td class="mono">GET /day/:d</td><td>the full day record: events, signals, KPIs, deviations, module runs, messages, decisions, pre-mortems, challenges, forecasts, actions, human actions, autonomy changes, Sentinel, findings, brief, scenarios, OKR flags, grid, LLM calls, metrics</td></tr>
    <tr><td class="mono">GET /day/:d/module/:m</td><td>one ModuleRun with its messages in/out, decisions, deviations, actions</td></tr>
    <tr><td class="mono">GET /series?metrics=&amp;from=&amp;to=</td><td>KPI and <code>ota.*</code> time series</td></tr>
    <tr><td class="mono">GET /ledger?q=&amp;class=&amp;module=&amp;status=&amp;from=&amp;to=</td><td>ledger search, newest first</td></tr>
    <tr><td class="mono">GET /decision/:id</td><td>decision with pre-mortem, challenges, actions, forecasts, assumptions</td></tr>
    <tr><td class="mono">GET /state</td><td>public state: assumptions, graveyard, scenarios, autonomy, calibration (aggregate), OKRs, Sentinel, findings, grid, briefs, queue</td></tr>
    <tr><td class="mono">POST /run, GET /run/status</td><td>background runs (admin token)</td></tr><tr><td class="mono">POST /human-input</td><td>real-human framing or verdicts (admin token)</td></tr></tbody></table></div>
    <h3>Ids</h3><p><code>&lt;PREFIX&gt;-&lt;DDDD&gt;-&lt;NNNN&gt;</code>: D decision, S signal, DV deviation, M message, F forecast, PM pre-mortem, CH challenge, A action, H human action, Q queue item, AU autonomy change, SN Sentinel finding, AN anomaly, SC scenario. Node ids are module ids, person ids, or toolkit pseudo-nodes.</p>
    <h3>The central record</h3><p><b>ModuleRun</b>: for this module on this day, the inputs, how it thought (tier, rules fired with bindings, LLM request/response, fallback, rationale) and the outputs. Messages carry <code>hops</code> — how many nodes information passed through before reaching a human (0 = a human’s own framing).</p>
    <h3>Daily metrics</h3><p>decision latency (first deviation → commit), loop speed (signal → execution), throughput, learning rate (Wright curve), hops to decider, real-human share, external signal share, human minutes, alerts routed/suppressed, pre-mortems run/skipped, ledger completeness, Brier 30d, LLM cost, bad news raised.</p>`,

  honest: ctx => `${kicker('honesty')}<h1>Deviations from the book</h1><p>The book is the specification; where this implementation departs from it, it says so.</p>
    <ol><li><b>Stand-ins do the human tasks</b> (the largest deviation, §13.17). Framing, judgment and accountability are performed by LLM personas — exactly the homogenisation risk the book warns about. Mitigations: real-human input through the viewer, <code>source: real_human | standin</code> on every human record, and the real-human share shown prominently.</li>
    <li><b>Culture is approximated.</b> Psychological safety and dissent are measured on module and stand-in behaviour (challenge rates, overrides, dissent diversity) and seeded by persona bias profiles. Tools cannot generate culture (ch 16).</li>
    <li><b>History is templated</b>, not LLM-generated: an authored storyline and phrase banks through the identical pipeline, so it is deterministic and costs nothing.</li>
    <li><b>Named rungs.</b> The book’s ladder has no named rungs; the five-rung scale is ours.</li>
    <li><b>Partial truth pipeline.</b> Metric owners and domains are loaded and shown, but “≥3 independent sources per domain”, the monthly situational-awareness review and shadow-group rotation are not enforced.</li>
    <li><b>Engine constants.</b> Sentinel, autonomy and lattice thresholds are engine constants, not company settings yet.</li>
    <li><b>Heuristic grid.</b> Success-grid cell health is a heuristic of tool coverage plus a few behaviour metrics.</li>
    <li><b>Rules propose scenarios</b> (large deviations, anomaly clusters), not an LLM scenario prompt.</li>
    <li><b>Experience proxy.</b> The learning rate uses the simulation day as the experience proxy for the Wright curve.</li></ol>
    <p>For this data: LLM mode <b>${esc(ctx.meta.llm_mode)}</b>; live models ${ctx.meta.live_llm_available ? 'available' : 'not available'} on this server.</p>`
};
