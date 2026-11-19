// Onboarding: what this is, the company, humans vs AI modules, an animated simulated day, how to
// read each view, an honest account of what is simulated, and a guided tour across real views.
import { api } from '../api.js';
import { esc, fmt, pct, md, minutes, $ } from '../util.js';
import { TICK, KIND_COL, PHASE_COL, TOOLS, conceptLink, TIER_COL } from '../catalog.js';
import { groupBy } from './module.js';

export async function render(el, ctx, rt) {
  if (rt.parts[0] === 'tour') { setTimeout(() => startTour(ctx), 0); }
  const M = ctx.meta, R = ctx.days;
  const people = M.people || [], mods = M.modules || [];
  const ai = mods.filter(m => !m.tool && (m.kind || 'ai_module') === 'ai_module'), tk = mods.filter(m => m.tool);
  const [state, rec] = await Promise.all([api.state().catch(() => null), R.count ? api.day(R.last).catch(() => null) : null]);
  if (!rt.isCurrent()) return;
  const desc = M.company?.description || '';
  const firstPara = (desc.split(/\n## /)[0] || '').replace(/^#.*\n/, '');
  const rhs = rec?.metrics?.real_human_share;
  const cnt = k => rec ? (Array.isArray(rec[k]) ? rec[k].length : 0) : 0;

  el.innerHTML = `<div class="wrap">
  <header class="hero"><div class="kicker">Start here · OTA simulation</div>
    <h1>${esc(M.company?.name || 'A company')}, run on the OTA loop</h1>
    <p class="lede">Risto Siilasmaa’s <i>OTA — Havaitse, Ajattele, Toimi</i> argues that organisations live or die by three muscles: how they <b style="color:var(--obs)">observe</b>, how they <b style="color:var(--think)">think</b>, and how they <b style="color:var(--act)">act</b>, and by how fast that loop turns and learns. This site runs those ideas as a working company, one simulated day at a time, and lets you inspect every observation, rule, model call, decision, gate and human signature.</p>
    <p class="muted" style="max-width:760px">${R.count ? `${R.count} days simulated (${esc(ctx.dateOf(R.first))} → ${esc(ctx.dateOf(R.last))}) · ${state?.ledger_size ?? '–'} decisions in the ledger · ${ai.length} AI modules · ${tk.length || Object.keys(TOOLS).length} toolkit components · ${people.length} humans.` : 'No days simulated yet.'}</p>
    <div class="cta"><button class="btn gold" id="tourbtn">▶ Take the 2-minute tour</button><a class="btn" href="#/birdseye/day/${R.last || 1}">Open the birdseye</a><a class="btn" href="#/docs">Read the docs</a><a class="btn" href="../">The book atlas ↗</a></div>
  </header>

  <section class="sect"><h2>The company</h2>
    <div class="grid g2"><div class="panel small">${md(firstPara) || '<p class="empty">No description.</p>'}${desc.length > firstPara.length + 40 ? `<details><summary>Full company profile</summary>${md(desc)}</details>` : ''}</div>
    <div class="panel"><h3>Today, day ${R.last || '–'}</h3>${rec ? `<div class="bigs">
      <div class="big"><span>Events</span><b>${cnt('events')}</b></div><div class="big"><span>Signals</span><b>${cnt('signals')}</b></div>
      <div class="big"><span>Module runs</span><b>${cnt('module_runs')}</b></div><div class="big"><span>Messages</span><b>${cnt('messages')}</b></div>
      <div class="big"><span>Decisions</span><b>${cnt('decisions')}</b></div><div class="big"><span>Human time</span><b>${minutes(rec.metrics?.human_minutes)}</b></div>
      <div class="big"><span>Real-human share</span><b style="color:${rhs > 0 ? 'var(--ok)' : 'var(--gold-2)'}">${pct(rhs, 1)}</b></div></div>` : '<p class="empty">–</p>'}
      ${rec?.brief ? `<p class="small" style="margin-top:10px"><span class="kicker">Signal brief</span><br>${esc(rec.brief.headline)}</p>` : ''}</div></div>
  </section>

  <section class="sect"><h2>Who does the work</h2>
    <p class="intro">The book’s AI-first organisation: a small leadership team, no middle management, domain-expert <b>validators</b> who gate claims, and humans at the <b>end nodes</b> where taste, voice and signatures matter. The working middle is AI modules, each an ${conceptLink('loop', 'OTA loop')} with its own decision classes and a named accountable human.</p>
    <div class="panel"><div class="row small" style="margin-bottom:10px"><span style="flex:${people.length};height:10px;background:${KIND_COL.leader};border-radius:3px"></span><span style="flex:${ai.length};height:10px;background:${KIND_COL.ai_module};border-radius:3px"></span><span style="flex:${tk.length};height:10px;background:${KIND_COL.toolkit};border-radius:3px"></span></div>
      <div class="legend" style="margin-bottom:12px"><span><i class="dot" style="background:${KIND_COL.leader}"></i>${people.length} humans</span><span><i class="dot" style="background:${KIND_COL.ai_module}"></i>${ai.length} AI modules</span><span><i class="dot" style="background:${KIND_COL.toolkit}"></i>${tk.length} toolkit modules</span></div>
      <div class="people-grid">${people.map(p => `<a class="person" href="${ctx.link.person(p.id)}" style="color:inherit;border-left:3px solid ${KIND_COL[p.role] || '#888'}"><b>${esc(p.name)}</b><span class="muted">${esc(p.title || p.role)}</span></a>`).join('')}</div>
      <div style="margin-top:14px">${groupBy(ai, m => m.org || 'other').map(([o, l]) => `<div class="row" style="margin:4px 0"><span class="kicker" style="width:110px">${esc(o)}</span>${l.map(m => `<a class="chip" href="${ctx.link.module(m.id, R.last)}">${(m.phases || []).slice(0, 1).map(p => `<i class="dot" style="background:${PHASE_COL[p]}"></i>`).join('')}${esc(m.name || m.id)}</a>`).join('')}</div>`).join('')}</div>
      <p class="tiny faint" style="margin-top:10px"><b>Important:</b> every human here is played by an LLM stand-in with a written persona and bias profile, unless a real person acts through the Run page. See “what is real” below.</p></div>
  </section>

  <section class="sect" data-tour="tick"><h2>How a simulated day flows</h2>
    <p class="intro">Every day runs the same tick, in this order. Press play, or click a step. Numbers are from day ${R.last || '–'}.</p>
    <div class="panel tick"><div class="row"><button class="btn" id="tplay">▶ Play</button><span class="tiny faint">the steps light up in the order the engine runs them</span></div>
      <div class="tickrail" id="trail">${TICK.map((s, k) => `<button class="tickstep" data-k="${k}"><span class="tiny" style="color:${PHASE_COL[s.ph] || 'var(--muted)'}">${k + 1} · ${esc(s.ph)}</span><b>${esc(s.n)}</b></button>`).join('')}</div>
      <div class="tickbody" id="tbody"></div></div>
  </section>

  <section class="sect"><h2>How to read each view</h2>
    <div class="howto">
      ${card('#/birdseye/day/' + (R.last || 1), 'Birdseye', 'The whole company on one day: the org graph (who talked to whom, who is under strain, which tier decided), a day scrubber with KPI sparklines and storyline markers, and the day’s big numbers.')}
      ${card(ctx.link.module(ctx.lastModule || ai[0]?.id || '', R.last), 'Module focus', 'One module on one day in three columns: what it saw (inputs), how it thought (rules fired, LLM prompt and response), what it did. ← → move a day.')}
      ${card('#/ledger', 'Ledger & decisions', 'Every decision with its seven fields and exit switches, its pre-mortem, challenges, gate colour, approvals (real human or stand-in) and the sealed outcome with its Brier score.')}
      ${card('#/health', 'Health', 'Is the loop itself healthy? Sentinel’s findings over all pathogens and AI failure modes, the success grid, autonomy over time, calibration, assumptions and the graveyard, scenarios, OKRs.')}
      ${card('#/people', 'People', 'The humans’ queues, minutes, approvals and overrides: how small the human footprint is, and where it is the bottleneck.')}
      ${card('#/run', 'Run', 'Advance the company with live models (admin token) and act as a real human: override a framing, approve or reject.')}
      ${card('#/docs', 'Docs', 'How the engine works inside: module Markdown, decision tables, the expression language, LLM tiers and budget, every toolkit component with its book origin.')}
    </div>
  </section>

  <section class="sect" id="honest"><h2>What is simulated, what is real, where we deviate from the book</h2>
    <div class="panel"><div class="tscroll"><table class="t honest"><tbody>
      <tr><th>The company</th><td>Fictional. Its games, people and numbers are invented to be credible, not to describe a real studio.</td></tr>
      <tr><th>The market</th><td>A deterministic, seeded model (installs, auctions, retention, monetisation, competitors, platform shocks). History events come from an authored storyline; live days can have LLM-written events.</td></tr>
      <tr><th>The engine</th><td><b>Real.</b> The toolkit, decision tables, ledger, gates, autonomy ladder, Sentinel and audit run as code over a real data store. What you see is what the engine computed.</td></tr>
      <tr><th>LLM calls</th><td>History days use a <b>scripted responder</b> (deterministic templates with the same JSON schemas). Live days call real models when the server has a key. Every call shows its tier, model and cost; ${M.llm_mode === 'scripted' ? 'this data is scripted' : 'this data mixes scripted history with live days'}.</td></tr>
      <tr><th>The humans</th><td><b>Stand-ins.</b> Framing, judgment and accountability — the three human tasks of ch 17 — are performed by LLM personas with bias profiles. This is the <b>largest deviation from the book</b> (ARCHITECTURE §13.17) and exactly the homogenisation risk it warns about. Mitigation: any real person with the admin token can act through <a href="#/run">Run</a>; such acts carry <span class="src-real">source: real_human</span> and the real-human share is shown everywhere.</td></tr>
      <tr><th>Culture</th><td>Cannot be generated by tools (ch 16). Psychological safety and dissent are approximated from persona bias profiles and measured on behaviour: challenge rates, overrides, dissent diversity.</td></tr>
      <tr><th>Autonomy rungs</th><td>The book’s ladder has no named rungs; the five rungs (inform → autonomous) are ours.</td></tr>
      <tr><th>History</th><td>Generated from templates and an authored storyline through the identical pipeline, not by LLMs.</td></tr>
      <tr><th>Tool rollout</th><td>The toolkit switches on in layers over the year (ledger first, monitors last), as the book prescribes; early flags are noisy because the ledger is young.</td></tr>
      <tr><th>Privacy</th><td>Individual human calibration is private to the person and never shown; only aggregates and AI profiles are public.</td></tr>
    </tbody></table></div><p class="tiny muted">More in <a href="#/docs/honest">Docs → Deviations</a> and the engine’s <code>docs/ARCHITECTURE.md</code>.</p></div>
  </section></div>`;

  $('#tourbtn', el).onclick = () => startTour(ctx);

  // animated tick
  let k = -1, timer = null, sub = 0;
  const live = s => rec && s.rec ? (Array.isArray(rec[s.rec]) ? rec[s.rec].length : 0) : null;
  const show = (i, subi = -1) => {
    k = i; const s = TICK[i];
    el.querySelectorAll('.tickstep').forEach((b, j) => { b.classList.toggle('on', j === i); b.classList.toggle('done', j < i); });
    const n = live(s);
    const tool = s.tool && TOOLS[s.tool];
    const glyph = tool ? (window.OTA_ID?.badge(tool.glyph, 44) || '') : '';
    $('#tbody', el).innerHTML = `<div class="row" style="align-items:flex-start;gap:14px;margin-top:6px">${glyph}<div style="flex:1;min-width:200px"><h3 style="margin:0 0 4px">${i + 1}. ${esc(s.n)}</h3><p style="margin:0">${esc(s.text)}</p>
      ${s.sub ? `<div class="subs">${s.sub.map((x, j) => `<span class="${j === subi ? 'on' : ''}" style="${s.id === 'think' ? `border-color:${TIER_COL[['rule', 'small', 'large', 'human'][j]]}` : ''}">${esc(x)}</span>`).join('<span style="border:0;padding:3px 0">→</span>')}</div>` : ''}
      ${n != null ? `<p class="small muted" style="margin-top:8px">On day ${R.last}: <b style="color:var(--gold-2)">${n}</b> ${esc(s.rec.replace(/_/g, ' '))}. ${s.id === 'think' ? `<a href="${ctx.link.module(ai[0]?.id || '', R.last)}">See one module think →</a>` : s.id === 'gov' ? '<a href="#/ledger">Open the ledger →</a>' : s.id === 'monitor' ? '<a href="#/health">Open health →</a>' : ''}</p>` : ''}
      ${tool ? `<p class="tiny"><a href="#/docs/tool-${s.tool}">${esc(tool.name)} in the docs</a> · ${conceptLink(tool.vocab, 'in the book atlas ↗')}</p>` : ''}</div></div>`;
  };
  el.querySelectorAll('.tickstep').forEach(b => b.onclick = () => { stop(); show(+b.dataset.k); });
  const stop = () => { clearInterval(timer); timer = null; $('#tplay', el).textContent = '▶ Play'; };
  $('#tplay', el).onclick = () => {
    if (timer) return stop();
    $('#tplay', el).textContent = '❚❚ Pause';
    if (k >= TICK.length - 1) k = -1;
    sub = -1; show(Math.max(0, k + (k < 0 ? 1 : 0)));
    timer = setInterval(() => {
      const s = TICK[k];
      if (s.sub && sub < s.sub.length - 1) { sub++; show(k, sub); return; }
      if (k >= TICK.length - 1) return stop();
      sub = -1; show(k + 1);
    }, 1100);
  };
  show(0);
  return () => clearInterval(timer);
}

function card(href, t, d) { return `<a class="panel" href="${href}"><h3 style="margin:0 0 4px">${t} →</h3><p class="small muted" style="margin:0">${d}</p></a>`; }

// ---------------- guided tour across the real views ----------------
let tourState = null;
export function startTour(ctx) {
  const R = ctx.days, d = R.last || 1;
  const firstAi = (ctx.meta.modules || []).find(m => !m.tool)?.id || '';
  const steps = [
    { hash: '#/start', sel: '[data-tour=tick]', t: 'One day, eleven steps', x: 'Every simulated day runs the same pipeline: the market moves, the observation stack flags what changed, modules think (rules first, models only when needed), governance checks each decision, people approve, actions land, and monitors watch the whole.' },
    { hash: `#/birdseye/day/${d}`, sel: '[data-tour=scrub]', t: 'The year at a glance', x: 'Scrub through every simulated day. Sparklines show revenue, players, installs, cost and human time; the marks above them are storyline events (gold), Sentinel findings (red) and autonomy changes (violet). ← → step a day.' },
    { hash: `#/birdseye/day/${d}`, sel: '[data-tour=graph]', t: 'The organisation on that day', x: 'Leaders on top, validators as gates, AI modules by organisation and OTA phase, the toolkit layer, and humans at the rim. Thicker edges carry more messages; the ring shows strain; the small dots show which tier (rule, small LLM, large LLM, human) decided. Click any node.' },
    { hash: ctx.link.module(firstAi, d), sel: '[data-tour=cols]', t: 'Inside one module', x: 'Inputs it received, how it thought (which decision-table row fired with which values, or the model prompt and response), and what it did. Every module, every day.' },
    { hash: '#/ledger', sel: '[data-tour=ledgerfilter]', t: 'The Decision Ledger', x: 'Every decision is a hypothesis with an exit. Search it, then open any entry to see its seven fields, its pre-mortem, challenges, gate colour, approvals and the sealed outcome.' },
    { hash: `#/health/day/${d}`, sel: '[data-tour=footprint]', t: 'Who is really deciding?', x: 'The real-human share comes first, on purpose: the human tasks here are mostly played by LLM stand-ins. Below: Sentinel’s scan for the six pathogens and thirteen AI failure modes, the success grid, autonomy and calibration.' },
    { hash: '#/run', sel: '[data-tour=human]', t: 'Step in', x: 'With the admin token you can advance the company with live models, or act as one of the humans: reframe a goal, approve or reject. Your act is recorded as a real-human act. That is the end of the tour.' }
  ];
  tourState = { i: 0, steps };
  const box = $('#tour');
  const go = i => {
    tourState.i = i; const s = steps[i];
    document.querySelectorAll('.tour-hl').forEach(e => e.classList.remove('tour-hl'));
    box.hidden = false;
    box.innerHTML = `<div class="kicker">Tour · ${i + 1} / ${steps.length}</div><h4>${esc(s.t)}</h4><div>${esc(s.x)}</div><div class="row"><button class="btn" id="tx">Close</button><span class="sp"></span>${i ? '<button class="btn" id="tp">‹ Back</button>' : ''}${i < steps.length - 1 ? '<button class="btn gold" id="tn">Next ›</button>' : '<button class="btn gold" id="tx2">Done</button>'}</div>`;
    $('#tx').onclick = end; $('#tp') && ($('#tp').onclick = () => go(i - 1)); $('#tn') && ($('#tn').onclick = () => go(i + 1)); $('#tx2') && ($('#tx2').onclick = end);
    const hl = () => { const e = document.querySelector(s.sel); if (e) { e.classList.add('tour-hl'); e.scrollIntoView({ block: 'center', behavior: 'smooth' }); } };
    if (location.hash !== s.hash) { ctx.onRoute = () => { if (tourState) setTimeout(hl, 120); }; location.hash = s.hash; } else hl();
  };
  const end = () => { box.hidden = true; tourState = null; ctx.onRoute = null; document.querySelectorAll('.tour-hl').forEach(e => e.classList.remove('tour-hl')); };
  go(0);
}
