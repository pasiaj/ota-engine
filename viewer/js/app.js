// Shell: navigation, hash router, shared context. Routes (all linkable):
//   #/start  #/start/tour  #/docs[/topic]  #/birdseye[/day/N]  #/module/<id>[/day/N]
//   #/decision/<id>  #/ledger[?q=&class=&module=&status=]  #/health[/day/N]
//   #/people[/<id>][/day/N]  #/run
import { api, BASE } from './api.js';
import { $, esc, store } from './util.js';
import { KIND_COL } from './catalog.js';

const VIEWS = {
  start: () => import('./views/start.js'),
  docs: () => import('./views/docs.js'),
  birdseye: () => import('./views/birdseye.js'),
  module: () => import('./views/module.js'),
  decision: () => import('./views/decision.js'),
  ledger: () => import('./views/decision.js'),
  health: () => import('./views/health.js'),
  people: () => import('./views/people.js'),
  person: () => import('./views/people.js'),
  run: () => import('./views/run.js')
};
const NAV = [['start', 'Start here'], ['birdseye', 'Birdseye'], ['module', 'Modules'], ['ledger', 'Ledger'], ['health', 'Health'], ['people', 'People'], ['run', 'Run'], ['docs', 'Docs']];

export const ctx = {
  meta: null, days: null, // {first,last,count}
  day: null,              // the day the user is looking at (sticky across views)
  node(id) { return this.nodes.get(id); },
  nodes: new Map(),
  // Name of any node id (module, person, toolkit pseudo-node).
  name(id) { const n = this.nodes.get(id); return n ? n.name : id; },
  dateOf(d) { const s = this.meta?.company?.start_date; if (!s) return ''; const t = new Date(s + 'T00:00:00Z'); t.setUTCDate(t.getUTCDate() + d - 1); return t.toISOString().slice(0, 10); },
  clampDay(d) { const r = this.days; if (!r || !r.count) return 1; return Math.max(r.first, Math.min(r.last, d | 0 || r.last)); },
  go(hash) { if (location.hash !== hash) location.hash = hash; else route(); },
  link: {
    module: (m, d) => `#/module/${encodeURIComponent(m)}${d ? '/day/' + d : ''}`,
    person: (p, d) => `#/people/${encodeURIComponent(p)}${d ? '/day/' + d : ''}`,
    node(id, d) { const n = ctx.nodes.get(id); return n && n.isPerson ? this.person(id, d) : this.module(id, d); },
    decision: id => `#/decision/${encodeURIComponent(id)}`,
    day: (d, v = 'birdseye') => `#/${v}/day/${d}`,
    docs: t => `#/docs/${t}`
  },
  help(topic, label = 'What is this?') { return `<a class="help" href="#/docs/${topic}" title="${esc(label)}">?</a>`; },
  nodeChip(id, d) {
    const n = this.nodes.get(id);
    const col = n ? (KIND_COL[n.kind] || 'var(--muted)') : 'var(--faint)';
    return `<a class="chip" href="${this.link.node(id, d)}"><i class="dot" style="background:${col}"></i>${esc(n ? n.name : id)}</a>`;
  }
};

function buildNodes(meta) {
  ctx.nodes.clear();
  for (const m of meta.modules || []) ctx.nodes.set(m.id, { id: m.id, name: m.name || m.id, kind: m.tool ? 'toolkit' : (m.kind || 'ai_module'), org: m.org, m });
  for (const p of meta.people || []) ctx.nodes.set(p.id, { id: p.id, name: p.name || p.id, kind: p.role, isPerson: true, p });
  const pseudo = ['reality-lattice', 'assumption-cycle', 'living-okrs', 'domain-expert-gate', 'double-loop', 'sentinel', 'signal-brief', 'adversarial-engine', 'meta-loop', 'scenario-tree'];
  for (const id of pseudo) if (!ctx.nodes.has(id)) ctx.nodes.set(id, { id, name: id.replace(/-/g, ' ').replace(/^./, c => c.toUpperCase()), kind: 'toolkit', pseudo: true });
}

function renderNav(active) {
  const logo = `<svg width="22" height="22" viewBox="0 0 48 48" fill="none" stroke-width="3.2" stroke-linecap="round"><path d="M24 6a18 18 0 0 1 15.6 9" stroke="#3fd0c9"/><path d="M41 30A18 18 0 0 1 24 42" stroke="#a98bff"/><path d="M12 37A18 18 0 0 1 8 16" stroke="#ff9a3d"/></svg>`;
  const name = ctx.meta?.company?.name || 'OTA simulation';
  const d = ctx.day;
  $('#navin').innerHTML = `<a class="brand" href="../" title="Back to the OTA book atlas">${logo}OTA</a><span class="sep"></span>` +
    `<a class="n${active === 'start' ? ' on' : ''}" href="#/start" style="color:var(--gold)">${esc(name)}</a>` +
    NAV.slice(1).map(([k, n]) => {
      let href = '#/' + k;
      if (d && (k === 'birdseye' || k === 'health')) href += '/day/' + d;
      if (k === 'module') href = ctx.lastModule ? ctx.link.module(ctx.lastModule, d) : '#/module';
      const on = k === active || (k === 'module' && active === 'module') || (k === 'ledger' && active === 'decision') || (k === 'people' && active === 'person');
      return `<a class="n${on ? ' on' : ''}" href="${href}">${n}</a>`;
    }).join('') +
    `<span class="dayctl" title="Data range served by the API">${ctx.runLive ? '<i class="live on"></i>running · ' : ''}<span class="lbl">days</span> ${ctx.days?.count ? ctx.days.first + '–' + ctx.days.last : '–'}</span>`;
}

let cleanup = null, seq = 0;
async function route() {
  const raw = location.hash.replace(/^#\/?/, '');
  const [path, qs] = raw.split('?');
  const parts = path.split('/').filter(Boolean).map(decodeURIComponent);
  let view = parts[0] || 'start';
  if (!VIEWS[view]) view = 'start';
  const params = { parts: parts.slice(1), query: new URLSearchParams(qs || '') };
  const di = parts.indexOf('day');
  if (di > 0 && parts[di + 1]) { params.day = ctx.clampDay(+parts[di + 1]); ctx.day = params.day; }
  const my = ++seq;
  if (cleanup) { try { cleanup(); } catch { /* ignore */ } cleanup = null; }
  renderNav(view);
  const el = $('#view');
  try {
    const mod = await VIEWS[view]();
    if (my !== seq) return;
    el.innerHTML = '<div class="wrap loading">Loading…</div>';
    const r = await mod.render(el, ctx, { view, ...params, isCurrent: () => my === seq });
    if (my !== seq) { if (typeof r === 'function') r(); return; }
    cleanup = typeof r === 'function' ? r : null;
    renderNav(view);
    document.title = (el.querySelector('h1')?.textContent || 'OTA') + ' · OTA simulation';
    if (!params.keepScroll) window.scrollTo(0, 0);
    ctx.onRoute?.(view, params);
  } catch (e) {
    console.error(e);
    if (my === seq) el.innerHTML = `<div class="wrap err"><h2>Something went wrong</h2><p>${esc(e.message || e)}</p><p class="muted">API: <code>${esc(BASE)}</code>. For local development add <code>?api=http://localhost:8080/ota/api/</code> to the URL.</p></div>`;
  }
}

function banner(html, info) { const b = $('#banner'); b.hidden = !html; b.className = info ? 'info' : ''; b.innerHTML = html || ''; }

async function boot() {
  try {
    const [meta, days] = await Promise.all([api.meta(), api.days()]);
    ctx.meta = meta; ctx.days = days.range || meta.days || { first: 1, last: 1, count: 0 };
    buildNodes(meta);
    ctx.runLive = !!meta.run?.running;
    if (!ctx.days.count) banner('The API is up but has no simulated days yet. Use <a href="#/run">Run</a> to advance the simulation.', true);
    else if (meta.llm_mode === 'scripted' || !meta.live_llm_available) banner(`LLM calls in this data are <b>scripted</b> (deterministic templates), not live models. <a href="#/docs/honest">What that means</a>`, true);
    const lm = store.get('ota.lastModule'); if (lm && ctx.nodes.has(lm)) ctx.lastModule = lm;
    ctx.day = ctx.days.last || 1;
  } catch (e) {
    renderNav('start');
    $('#view').innerHTML = `<div class="wrap err"><h1>Cannot reach the simulation API</h1><p>${esc(e.message)}</p><p class="muted">Tried <code>${esc(BASE)}meta</code>. The viewer expects the API at <code>../api/</code> relative to this page; override with <code>?api=…</code>.</p></div>`;
    return;
  }
  window.addEventListener('hashchange', route);
  route();
}

// Refresh cached data after a run finishes (called by the run view / poller).
export async function reloadMeta() {
  api.invalidate();
  const [meta, days] = await Promise.all([api.meta(), api.days()]);
  ctx.meta = meta; ctx.days = days.range || meta.days; buildNodes(meta);
  ctx.runLive = !!meta.run?.running; renderNav(location.hash.split('/')[1] || 'start');
}

boot();
