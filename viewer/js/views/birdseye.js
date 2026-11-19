// Birdseye: the whole company on one day — org graph, day scrubber with KPI sparklines and
// event/sentinel markers, big-number summary, and today's notable records.
import { api } from '../api.js';
import { esc, fmt, pct, usd, minutes, spark, stackBars, humanize, $ } from '../util.js';
import { TIER_COL, TIERS, KIND_COL, PHASE_COL, NODE_TOOL, TOOLS } from '../catalog.js';
import { headline } from '../kpis.js';
import * as DI from '../dayindex.js';

const PHASE_ORDER = ['observe', 'think', 'act'];

export async function render(el, ctx, rt) {
  const R = ctx.days;
  if (!R.count) { el.innerHTML = `<div class="wrap"><div class="ph"><h1>Birdseye</h1></div><p class="empty">No days simulated yet.</p></div>`; return; }
  let day = rt.day || ctx.day || R.last;
  const H = await headline();
  el.innerHTML = `<div class="wrap">
    <div class="ph"><div><div class="kicker">Birdseye · the whole company</div><h1 id="bh">Day ${day}</h1><p class="sub" id="bsub"></p></div>
      <div class="tools"><a class="btn" id="bmod" href="#">Module focus →</a><a class="btn" id="bhealth" href="#">Health →</a></div></div>
    <section class="panel scrub" data-tour="scrub">${ctx.help('birdseye-scrubber', 'How to read the scrubber')}
      <div class="ctl"><button class="btn" id="prev" title="Previous day (←)">‹</button><button class="btn" id="play" title="Play">▶</button><button class="btn" id="next" title="Next day (→)">›</button>
        <span class="date" id="bdate"></span><input type="range" id="slider" min="${R.first}" max="${R.last}" value="${day}" aria-label="Day"></div>
      <div class="markers" id="markers" title="Events (gold), Sentinel findings (red), autonomy changes (violet)"></div>
      <div class="sparks" id="sparks"></div>
      <div class="legend" style="margin-top:6px"><span><i class="dot" style="background:#f1d27a"></i>storyline / event</span><span><i class="dot" style="background:var(--danger)"></i>Sentinel finding</span><span><i class="dot" style="background:#a98bff"></i>autonomy change</span><span class="faint" id="idxp"></span></div>
    </section>
    <section class="panel" style="margin-top:14px" data-tour="bigs">${ctx.help('birdseye-numbers')}<div class="bigs" id="bigs"></div></section>
    <section class="panel" style="margin-top:14px" data-tour="graph"><header><h3>Organisation on day <span id="gday"></span></h3>
      <div class="r legend"><span><i class="dot" style="background:${KIND_COL.leader}"></i>leader</span><span><i class="dot" style="background:${KIND_COL.validator}"></i>validator (gate)</span><span><i class="dot" style="background:${KIND_COL.ai_module}"></i>AI module</span><span><i class="dot" style="background:${KIND_COL.toolkit}"></i>toolkit</span><span><i class="dot" style="background:${KIND_COL.end_node}"></i>end node</span>${ctx.help('birdseye-graph', 'How to read the graph')}</div></header>
      <div class="graphwrap" id="graph"></div>
      <div class="legend" style="margin-top:6px"><span>ring = health (green calm · yellow deviations · red over-budget alerts / red gate)</span><span>badges = tier used: ${TIERS.map(t => `<span><i class="dot" style="background:${TIER_COL[t]}"></i>${t}</span>`).join(' ')}</span><span>edge width = messages</span></div>
    </section>
    <div class="grid g3" style="margin-top:14px" id="today"></div>
  </div>`;

  const tip = document.createElement('div'); tip.className = 'tip'; tip.hidden = true; document.body.appendChild(tip);
  const idxOf = d => H.days.indexOf(d);
  let markersHTML = '';

  function drawSparks() {
    const i = idxOf(day);
    const marks = [];
    const rows = H.rows.map(r => ({ label: r.label, vals: r.vals, color: r.color, fmt: r.key === 'revenue' ? v => '€' + fmt(v) : fmt }));
    rows.push({ label: 'LLM cost / day', vals: H.ota.llm_cost_usd, color: '#a98bff', fmt: usd });
    rows.push({ label: 'Human minutes', vals: H.ota.human_minutes, color: '#f1d27a', fmt: minutes });
    const tierVals = TIERS.map(t => ({ color: TIER_COL[t], vals: H.days.map(d => DI.get(d)?.tiers?.[t] || 0) }));
    const anyTier = DI.size() > 0;
    $('#sparks', el).innerHTML = rows.map(r => `<div class="s"><div class="l"><span>${r.label}</span><b>${r.fmt(r.vals[i])}</b></div>${spark(r.vals, { cursor: i, color: r.color, marks })}</div>`).join('') +
      `<div class="s"><div class="l"><span>Decisions by tier</span><b>${fmt(H.ota.decisions[i])}</b></div>${anyTier ? stackBars(tierVals, { w: 400, h: 34, cursor: i }) : spark(H.ota.decisions, { cursor: i, color: '#5fd38d' })}</div>`;
    $$sparkClicks();
  }
  function $$sparkClicks() {
    el.querySelectorAll('#sparks .s').forEach(s => {
      s.onclick = ev => { const r = s.getBoundingClientRect(); const f = (ev.clientX - r.left) / r.width; setDay(H.days[Math.round(f * (H.days.length - 1))]); };
    });
  }
  function drawMarkers() {
    const span = Math.max(1, R.last - R.first);
    let h = '';
    for (let d = R.first; d <= R.last; d++) {
      const s = DI.get(d); if (!s) continue;
      const x = ((d - R.first) / span * 100).toFixed(2) + '%';
      const story = s.ev.filter(e => e.g === 'storyline' || e.g === 'llm');
      if (story.length) h += `<i style="left:${x};background:#f1d27a" data-d="${d}" title="Day ${d}: ${esc(story.map(e => e.t).join(' · '))}"></i>`;
      if (s.sn.length) h += `<i style="left:${x};background:var(--danger);top:7px;height:6px" data-d="${d}" title="Day ${d}: Sentinel ${esc(s.sn.map(humanize).join(', '))}"></i>`;
      if (s.au) h += `<i style="left:${x};background:#a98bff;top:0;height:5px" data-d="${d}" title="Day ${d}: ${s.au} autonomy change(s)"></i>`;
    }
    if (h !== markersHTML) { markersHTML = h; $('#markers', el).innerHTML = h; }
    $('#idxp', el).textContent = DI.size() < R.count ? `indexing days ${DI.size()}/${R.count}…` : '';
  }
  el.querySelector('#markers').onclick = e => { const d = e.target.dataset?.d; if (d) setDay(+d); };

  let rec = null;
  async function loadDay(d) {
    day = d; ctx.day = d;
    history.replaceState(null, '', `#/birdseye/day/${d}`);
    $('#slider', el).value = d;
    $('#bh', el).textContent = `Day ${d}`;
    $('#bdate', el).textContent = ctx.dateOf(d);
    $('#gday', el).textContent = d;
    $('#bmod', el).href = ctx.lastModule ? ctx.link.module(ctx.lastModule, d) : '#/module';
    $('#bhealth', el).href = `#/health/day/${d}`;
    drawSparks();
    rec = await api.day(d);
    if (day !== d || !rt.isCurrent()) return;
    DI.put(d, rec);
    $('#bsub', el).innerHTML = `${esc(rec.date)} · ${esc(rec.weekday)} · cadences: ${rec.cadences.map(esc).join(', ')} · ${rec.tools_enabled.length} tools on · LLM ${esc(rec.llm_mode)}`;
    drawBigs(rec, d); drawGraph(rec, d); drawToday(rec, d);
  }
  function drawBigs(r, d) {
    const i = idxOf(d), m = r.metrics || {};
    const byT = m.decisions_by_tier || {};
    const rev = H.rows.find(x => x.key === 'revenue'), dau = H.rows.find(x => x.key === 'dau');
    const delta = (row) => { if (!row || i < 7 || row.vals[i] == null || row.vals[i - 7] == null) return ''; const c = row.vals[i] / row.vals[i - 7] - 1; return `<small style="color:${c >= 0 ? 'var(--ok)' : 'var(--danger)'}">${c >= 0 ? '+' : ''}${(c * 100).toFixed(1)}% vs w-1</small>`; };
    const supp = m.alerts_suppressed || 0;
    $('#bigs', el).innerHTML = [
      rev && `<div class="big"><span>Revenue</span><b>€${fmt(rev.vals[i])}</b>${delta(rev)}</div>`,
      dau && `<div class="big"><span>DAU</span><b>${fmt(dau.vals[i])}</b>${delta(dau)}</div>`,
      `<div class="big"><span>Decisions</span><b>${fmt(m.decisions_committed ?? r.decisions.length)}</b><small>${TIERS.filter(t => byT[t]).map(t => `<span style="color:${TIER_COL[t]}">${byT[t]} ${t}</span>`).join(' · ') || 'none committed'}</small></div>`,
      `<div class="big"><span>Module runs</span><b>${r.module_runs.length}</b><small>${r.llm_calls.length} LLM calls · ${usd(m.llm_cost_usd)}</small></div>`,
      `<div class="big"><span>Alerts</span><b>${fmt(m.alerts_routed || 0)}</b><small style="color:${supp ? 'var(--danger)' : 'inherit'}">${supp} over budget</small></div>`,
      `<div class="big"><span>Human time</span><b>${minutes(m.human_minutes)}</b><small>${Object.keys(r.human_load || {}).length} people worked</small></div>`,
      `<div class="big"><span>Real-human share</span><b style="color:${m.real_human_share > 0 ? 'var(--ok)' : 'var(--gold-2)'}">${pct(m.real_human_share, 1)}</b><small>of human acts, 30 days</small></div>`,
      `<div class="big"><span>Sentinel</span><b style="color:${r.sentinel.length ? 'var(--danger)' : 'inherit'}">${r.sentinel.length}</b><small>${r.sentinel.map(s => humanize(s.kind.name)).join(', ') || 'no new findings'}</small></div>`
    ].filter(Boolean).join('');
  }

  // ---------- org graph ----------
  function layout(r) {
    const W = 1200, nodes = [], byId = new Map();
    const add = (n) => { nodes.push(n); byId.set(n.id, n); };
    const people = ctx.meta.people || [], mods = ctx.meta.modules || [];
    const ai = mods.filter(m => !m.tool && (m.kind || 'ai_module') === 'ai_module');
    const tk = mods.filter(m => m.tool || m.kind === 'toolkit');
    let y = 40;
    const rowOf = (list, yy, kind, r0) => { const n = list.length; list.forEach((p, k) => add({ id: p.id, name: p.name || p.id, kind: kind || p.role, x: (k + .5) / n * W, y: yy, r: r0 })); };
    const bands = [];
    const leaders = people.filter(p => p.role === 'leader');
    bands.push({ y, label: 'Leadership' }); rowOf(leaders, y + 26, 'leader', 17); y += 76;
    const vals = people.filter(p => p.role === 'validator');
    if (vals.length) { bands.push({ y, label: 'Validators · Domain Expert Gate' }); rowOf(vals, y + 24, 'validator', 13); y += 70; }
    // AI modules: columns by org, rows by first phase
    const orgs = [...new Set(ai.map(m => m.org || 'other'))];
    const cell = {}; for (const m of ai) { const ph = PHASE_ORDER.find(p => (m.phases || []).includes(p)) || 'think'; (cell[(m.org || 'other') + '|' + ph] ||= []).push(m); }
    const colW = orgs.map(o => Math.max(1, ...PHASE_ORDER.map(p => (cell[o + '|' + p] || []).length)));
    const totW = colW.reduce((a, b) => a + b, 0) || 1;
    const top = y; bands.push({ y, label: 'AI organisation', orgs: [] });
    let x0 = 70;
    const usable = W - 90;
    orgs.forEach((o, k) => {
      const w = colW[k] / totW * usable;
      bands[bands.length - 1].orgs.push({ x: x0 + w / 2, name: o });
      PHASE_ORDER.forEach((p, pi) => {
        const list = cell[o + '|' + p] || [];
        list.forEach((m, j) => add({ id: m.id, name: m.name || m.id, kind: 'ai_module', org: o, phase: p, x: x0 + (j + .5) / list.length * w, y: top + 44 + pi * 62 + (list.length > 3 && j % 2 ? 14 : 0), r: 12 }));
      });
      x0 += w;
    });
    const phaseY = PHASE_ORDER.map((p, pi) => ({ p, y: top + 44 + pi * 62 }));
    y = top + 44 + 3 * 62 + 4;
    // toolkit layer: company toolkit modules + pseudo-nodes that appear in messages
    const tkNodes = tk.map(m => ({ id: m.id, name: m.name || m.id }));
    const seen = new Set(tkNodes.map(t => t.id));
    for (const msg of r.messages) for (const id of [msg.from, ...msg.to]) if (!byId.has(id) && !seen.has(id) && ctx.node(id)?.kind === 'toolkit') { seen.add(id); tkNodes.push({ id, name: ctx.name(id) }); }
    if (tkNodes.length) {
      bands.push({ y, label: 'OTA toolkit' });
      const perRow = Math.min(tkNodes.length, 12); const rows = Math.ceil(tkNodes.length / perRow);
      tkNodes.forEach((t, k) => { const row = Math.floor(k / perRow), inRow = Math.min(perRow, tkNodes.length - row * perRow); add({ id: t.id, name: t.name, kind: 'toolkit', x: ((k % perRow) + .5) / inRow * W, y: y + 26 + row * 46, r: 9 }); });
      y += 26 + rows * 46;
    }
    const ends = people.filter(p => p.role !== 'leader' && p.role !== 'validator');
    if (ends.length) { bands.push({ y, label: 'End nodes · the rim' }); rowOf(ends, y + 24, null, 12); y += 66; }
    return { nodes, byId, bands, phaseY, H: y + 6, W };
  }

  function health(r, id) {
    const runs = r.module_runs.find(x => x.module === id);
    const routed = r.deviations.filter(d => d.routed_to.includes(id));
    const supp = routed.filter(d => d.suppressed_by_budget).length;
    const red = r.decisions.some(d => (d.module === id || d.gate?.validator === id) && d.gate?.color === 'red');
    const fails = runs?.think?.rules_failed?.length || 0;
    const q = r.queue_open?.[id] || 0;
    if (supp || red) return { c: 'var(--danger)', why: `${supp} over-budget alert(s)${red ? ', red gate' : ''}` };
    if (routed.length || fails || q > 3) return { c: '#ffc94a', why: `${routed.length} deviation(s)${fails ? `, ${fails} failed rule(s)` : ''}${q ? `, ${q} queued` : ''}` };
    return { c: 'var(--ok)', why: 'calm' };
  }

  function drawGraph(r, d) {
    const L = layout(r);
    const edges = new Map();
    for (const m of r.messages) for (const to of m.to) {
      if (!L.byId.has(m.from) || !L.byId.has(to) || m.from === to) continue;
      const k = m.from + '>' + to; const e = edges.get(k) || { a: m.from, b: to, n: 0, intents: {} }; e.n++; e.intents[m.intent] = (e.intents[m.intent] || 0) + 1; edges.set(k, e);
    }
    // decisions → accountable humans / approvals as implied edges (dimmer)
    const deg = new Map(); for (const e of edges.values()) { deg.set(e.a, (deg.get(e.a) || 0) + e.n); deg.set(e.b, (deg.get(e.b) || 0) + e.n); }
    const tiersBy = {};
    for (const run of r.module_runs) { (tiersBy[run.module] ||= {}); if (run.think?.tier && (run.think.tier !== 'rule' || run.outputs?.decisions?.length)) tiersBy[run.module][run.think.tier] = (tiersBy[run.module][run.think.tier] || 0) + Math.max(1, run.outputs?.decisions?.length || 0); }
    for (const h of r.human_actions) { (tiersBy[h.person] ||= {}); tiersBy[h.person].human = (tiersBy[h.person].human || 0) + 1; }
    const maxN = Math.max(1, ...[...edges.values()].map(e => e.n));
    const ed = [...edges.values()].sort((a, b) => a.n - b.n).map(e => {
      const A = L.byId.get(e.a), B = L.byId.get(e.b);
      const mx = (A.x + B.x) / 2, my = (A.y + B.y) / 2, dx = B.x - A.x, dy = B.y - A.y;
      const cx = mx - dy * .18, cy = my + dx * .18;
      const w = 0.8 + 5 * Math.sqrt(e.n / maxN);
      const col = e.intents.escalate || e.intents.challenge ? '#ff9a3d' : e.intents.propose || e.intents.request ? '#f1d27a' : '#8cc8ff';
      const p = `M${A.x.toFixed(1)} ${A.y.toFixed(1)}Q${cx.toFixed(1)} ${cy.toFixed(1)} ${B.x.toFixed(1)} ${B.y.toFixed(1)}`;
      return `<path class="edge" d="${p}" stroke="${col}" stroke-opacity=".22" stroke-width="${w.toFixed(2)}"><title>${esc(ctx.name(e.a))} → ${esc(ctx.name(e.b))}: ${e.n} message(s) (${Object.entries(e.intents).map(([k, v]) => v + ' ' + k).join(', ')})</title></path>` +
        (e.n >= Math.max(2, maxN * .25) ? `<path class="edge pulse" d="${p}" stroke="${col}" stroke-opacity=".8" stroke-width="${Math.min(w, 2.5).toFixed(2)}"/>` : '');
    }).join('');
    const bands = L.bands.map(b => `<text class="band" x="8" y="${b.y + 6}">${esc(b.label)}</text><line x1="0" x2="${L.W}" y1="${b.y - 6}" y2="${b.y - 6}" stroke="var(--line-2)"/>` +
      (b.orgs || []).map(o => `<text class="band" x="${o.x}" y="${b.y + 6}" text-anchor="middle" fill="var(--muted)">${esc(o.name)}</text>`).join('')).join('') +
      L.phaseY.map(p => `<text x="8" y="${p.y + 4}" font-size="10" fill="${PHASE_COL[p.p]}" opacity=".8">${p.p}</text>`).join('');
    const showLabels = L.nodes.length <= 80;
    const nodes = L.nodes.map(n => {
      const k = n.kind, col = KIND_COL[k] || '#888';
      const act = deg.get(n.id) || 0;
      const r0 = n.r + Math.min(8, Math.sqrt(act) * 1.6);
      const hl = health(r, n.id);
      const tiers = tiersBy[n.id] || {};
      const badges = TIERS.filter(t => tiers[t]).map((t, j) => `<circle cx="${(n.x + r0 * .75 + j * 7).toFixed(1)}" cy="${(n.y - r0 * .8).toFixed(1)}" r="3.6" fill="${TIER_COL[t]}" stroke="#07080b" stroke-width="1"><title>${tiers[t]} × ${t}</title></circle>`).join('');
      let shape;
      if (k === 'validator') shape = `<path class="body" d="M${n.x} ${n.y - r0}L${n.x + r0} ${n.y}L${n.x} ${n.y + r0}L${n.x - r0} ${n.y}Z" fill="${col}" fill-opacity=".18" stroke="${col}" stroke-width="1.6"/>`;
      else if (k === 'toolkit') { const tid = NODE_TOOL[n.id] || n.id.replace(/-/g, '_'); const on = !TOOLS[tid] || r.tools_enabled.includes(tid); shape = `<rect class="body" x="${n.x - r0}" y="${n.y - r0}" width="${2 * r0}" height="${2 * r0}" rx="3" fill="${col}" fill-opacity="${on ? .2 : .03}" stroke="${col}" stroke-opacity="${on ? 1 : .35}" stroke-dasharray="${on ? '' : '2 2'}" stroke-width="1.4"/>`; }
      else shape = `<circle class="body" cx="${n.x}" cy="${n.y}" r="${r0.toFixed(1)}" fill="${col}" fill-opacity=".16" stroke="${col}" stroke-width="1.6"/>`;
      const ring = k === 'toolkit' ? '' : `<circle cx="${n.x}" cy="${n.y}" r="${(r0 + 3.5).toFixed(1)}" fill="none" stroke="${hl.c}" stroke-width="2" stroke-opacity=".85"/>`;
      const label = showLabels ? `<text x="${n.x}" y="${(n.y + r0 + 13).toFixed(1)}" font-size="${k === 'toolkit' ? 9 : 10.5}" text-anchor="middle" fill="${k === 'toolkit' ? 'var(--muted)' : 'var(--text)'}">${esc(short(n.name, k === 'toolkit' ? 14 : 18))}</text>` : '';
      return `<g class="node" data-id="${esc(n.id)}" data-why="${esc(hl.why)}">${ring}${shape}${badges}${label}</g>`;
    }).join('');
    $('#graph', el).innerHTML = `<svg class="org" viewBox="0 0 ${L.W} ${L.H}" role="img" aria-label="Organisation graph for day ${d}">${bands}<g>${ed}</g>${nodes}</svg>`;
    const svg = $('#graph svg', el);
    svg.addEventListener('click', e => { const g = e.target.closest('.node'); if (g) location.hash = ctx.link.node(g.dataset.id, d); });
    svg.addEventListener('mousemove', e => {
      const g = e.target.closest('.node'); if (!g) { tip.hidden = true; return; }
      const id = g.dataset.id, n = ctx.node(id), run = r.module_runs.find(x => x.module === id);
      const out = r.messages.filter(m => m.from === id).length, inn = r.messages.filter(m => m.to.includes(id)).length;
      const decs = r.decisions.filter(x => x.module === id);
      tip.innerHTML = `<b>${esc(n?.name || id)}</b> <span class="faint">${esc(humanize(n?.kind || ''))}${n?.org ? ' · ' + esc(n.org) : ''}</span><br>` +
        `${inn} in · ${out} out · ${decs.length} decision(s)` + (run ? `<br>think: <span class="tier t-${run.think.tier}">${run.think.tier}</span> ${esc(short(run.think.rationale || '', 140))}` : '') +
        (r.human_load?.[id] ? `<br>${minutes(r.human_load[id])} of work, ${r.queue_open?.[id] || 0} open in queue` : '') + `<br><span class="faint">health: ${esc(g.dataset.why)}</span>`;
      tip.hidden = false;
      const x = Math.min(e.clientX + 14, innerWidth - 310), y = Math.min(e.clientY + 14, innerHeight - tip.offsetHeight - 8);
      tip.style.left = x + 'px'; tip.style.top = y + 'px';
    });
    svg.addEventListener('mouseleave', () => { tip.hidden = true; });
  }

  function drawToday(r, d) {
    const L = ctx.link;
    const ev = r.events.length ? r.events.map(e => `<li><b>${esc(e.title)}</b> <span class="chip">${esc(e.kind)}</span> <span class="chip">${esc(e.generated_by)}</span><div class="meta">${esc(e.text)}</div></li>`).join('') : '<li class="empty">A quiet day in the market.</li>';
    const brief = r.brief ? `<p style="margin:0 0 6px"><b>${esc(r.brief.headline)}</b></p><ul class="list">${r.brief.items.map(i => `<li>${esc(i)}</li>`).join('')}</ul><p class="tiny faint">drafted by the ${esc(r.brief.tier)} tier${r.brief.approved_by ? ' · approved by ' + esc(ctx.name(r.brief.approved_by)) : ''}</p>` : '<p class="empty">No brief (tool not on yet, or nothing to say).</p>';
    const decs = [...r.decisions].sort((a, b) => b.stakes_eur - a.stakes_eur);
    const dl = decs.length ? decs.slice(0, 10).map(x => `<li><a href="${L.decision(x.id)}">${esc(x.what)}</a><div class="meta"><span class="tier t-${x.tier}">${x.tier}</span> ${esc(ctx.name(x.module))} · €${fmt(x.stakes_eur)} · <span style="color:var(--muted)">${esc(x.status)}</span>${x.gate ? ` · gate <b style="color:${x.gate.color === 'red' ? 'var(--danger)' : x.gate.color === 'yellow' ? '#ffc94a' : 'var(--ok)'}">${x.gate.color}</b>` : ''}</div></li>`).join('') + (decs.length > 10 ? `<li><a href="#/ledger?from=${d}&to=${d}">all ${decs.length} decisions of the day →</a></li>` : '') : '<li class="empty">No decisions proposed.</li>';
    const sn = r.sentinel.length ? r.sentinel.map(s => `<li><b style="color:var(--danger)">${esc(humanize(s.kind.name))}</b> <span class="chip">${esc(humanize(s.kind.category))}</span> <span class="chip">${esc(s.severity)}</span><div class="meta">${esc(s.signature)}</div></li>`).join('') : '';
    const hum = r.human_actions.length ? r.human_actions.slice(0, 8).map(h => `<li>${ctx.nodeChip(h.person, d)} ${esc(h.verdict || h.kind)} <span class="faint">${esc(humanize(h.kind))}</span> <span class="${h.source === 'real_human' ? 'src-real' : 'src-standin'}">${h.source === 'real_human' ? 'REAL HUMAN' : 'stand-in'}</span>${h.decision ? ` · <a href="${L.decision(h.decision)}">${esc(h.decision)}</a>` : ''}<div class="meta">${minutes(h.minutes_spent)} · ${esc(short(h.reason || '', 120))}</div></li>`).join('') : '<li class="empty">No human acted today.</li>';
    $('#today', el).innerHTML = `
      <section class="panel"><h3>Market &amp; storyline</h3>${ctx.help('tick')}<ul class="list">${ev}</ul></section>
      <section class="panel"><h3>Signal Brief</h3>${ctx.help('tool-brief')}${brief}</section>
      <section class="panel"><h3>Decisions proposed today</h3>${ctx.help('decisions')}<ul class="list">${dl}</ul></section>
      <section class="panel"><h3>Humans today</h3>${ctx.help('humans')}<ul class="list">${hum}</ul></section>
      ${sn ? `<section class="panel"><h3>Sentinel</h3>${ctx.help('sentinel')}<ul class="list">${sn}</ul></section>` : ''}
      <section class="panel"><h3>Deviations (Reality Lattice)</h3>${ctx.help('tool-lattice')}<ul class="list">${r.deviations.length ? r.deviations.slice(0, 8).map(v => `<li><b>${esc(v.metric)}</b> ${v.z >= 0 ? '+' : ''}${fmt(v.z, 1)}σ · L${v.level}${v.suppressed_by_budget ? ' <b style="color:var(--danger)">over budget</b>' : ''}<div class="meta">${esc(v.interpretation || '')} → ${v.routed_to.map(x => ctx.nodeChip(x, d)).join(' ')}</div></li>`).join('') : '<li class="empty">No deviations detected.</li>'}</ul></section>`;
  }

  function setDay(d) { d = ctx.clampDay(d); if (d !== day || !rec) loadDay(d); }
  $('#slider', el).addEventListener('input', e => { $('#bdate', el).textContent = `day ${e.target.value} · ${ctx.dateOf(+e.target.value)}`; });
  $('#slider', el).addEventListener('change', e => setDay(+e.target.value));
  $('#prev', el).onclick = () => setDay(day - 1);
  $('#next', el).onclick = () => setDay(day + 1);
  let timer = null;
  $('#play', el).onclick = () => {
    if (timer) { clearInterval(timer); timer = null; $('#play', el).textContent = '▶'; return; }
    $('#play', el).textContent = '❚❚';
    timer = setInterval(() => { if (day >= R.last) { clearInterval(timer); timer = null; $('#play', el).textContent = '▶'; return; } setDay(day + 1); }, 900);
  };
  const onKey = e => { if (e.target.closest('input,textarea,select')) return; if (e.key === 'ArrowLeft') setDay(day - 1); if (e.key === 'ArrowRight') setDay(day + 1); };
  document.addEventListener('keydown', onKey);
  const unsub = DI.subscribe(() => { drawMarkers(); drawSparks(); });
  await loadDay(day);
  drawMarkers();
  DI.scan(R);
  return () => { document.removeEventListener('keydown', onKey); unsub(); tip.remove(); if (timer) clearInterval(timer); };
}

function short(s, n) { s = String(s || ''); return s.length > n ? s.slice(0, n - 1) + '…' : s; }
