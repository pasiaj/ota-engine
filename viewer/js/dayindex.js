// Background index over all day files: events, decisions by tier, sentinel, real-human actions.
// The API has no per-day summary endpoint, so we fetch day files slowly (a few at a time) once
// per session and keep only a compact summary (sessionStorage, guarded). Views subscribe to
// progress and redraw markers/bars as the index fills.
import { api } from './api.js';

const KEY = 'ota.dayindex.v3';
const idx = new Map();     // day → summary
const subs = new Set();
let running = false, scannedTo = 0, generation = 0;

function load(last) {
  try {
    const j = JSON.parse(sessionStorage.getItem(KEY) || 'null');
    if (j && j.base === location.search) for (const [d, s] of j.days) if (d <= last) idx.set(d, s);
  } catch { /* ignore */ }
}
function save() { try { sessionStorage.setItem(KEY, JSON.stringify({ base: location.search, days: [...idx] })); } catch { /* quota: fine */ } }

export function summarise(r) {
  const tiers = {};
  for (const d of r.decisions || []) tiers[d.tier] = (tiers[d.tier] || 0) + 1;
  return {
    date: r.date,
    ev: (r.events || []).map(e => ({ t: e.title, k: e.kind, g: e.generated_by, id: e.id })).slice(0, 12),
    tiers,
    sn: (r.sentinel || []).map(s => s.kind?.name),
    real: (r.human_actions || []).filter(h => h.source === 'real_human').length,
    hum: (r.human_actions || []).length,
    au: (r.autonomy_changes || []).length,
    grid: !!r.grid,
    // per node: [tier index or -1, decisions, messages in+out, deviations routed]
    m: perNode(r),
    hl: r.human_load || {},
    hv: humanVerdicts(r)
  };
}
const TI = { rule: 0, small: 1, large: 2, human: 3 };
function perNode(r) {
  const m = {};
  const g = id => (m[id] ||= [-1, 0, 0, 0]);
  for (const run of r.module_runs || []) { const x = g(run.module); x[0] = Math.max(x[0], TI[run.think?.tier] ?? 0); }
  for (const d of r.decisions || []) g(d.module)[1]++;
  for (const msg of r.messages || []) { g(msg.from)[2]++; for (const t of msg.to) g(t)[2]++; }
  for (const v of r.deviations || []) for (const t of v.routed_to) g(t)[3]++;
  for (const h of r.human_actions || []) { const x = g(h.person); x[0] = 3; x[1]++; }
  return m;
}
function humanVerdicts(r) {
  const o = {};
  for (const h of r.human_actions || []) {
    const x = (o[h.person] ||= {});
    const k = h.kind === 'framing' ? 'framing' : (h.verdict || h.kind);
    x[k] = (x[k] || 0) + 1;
    if (h.source === 'real_human') x.real = (x.real || 0) + 1;
  }
  for (const d of [...(r.decisions || []), ...(r.decision_updates || [])]) for (const ov of d.overrides || []) if (ov.day === r.day) { const x = (o[ov.by] ||= {}); x.override = (x.override || 0) + 1; }
  return o;
}
export function put(day, rec) { if (!idx.has(day)) { idx.set(day, summarise(rec)); emit(); } }
export const get = d => idx.get(d);
export const size = () => idx.size;
export function subscribe(f) { subs.add(f); return () => subs.delete(f); }
let emitT = null;
function emit() { if (emitT) return; emitT = setTimeout(() => { emitT = null; subs.forEach(f => { try { f(); } catch (e) { console.error(e); } }); }, 250); }

export async function scan(range) {
  if (!range || !range.count) return;
  if (!scannedTo) load(range.last);
  if (running) return;
  running = true; const gen = ++generation;
  const todo = [];
  for (let d = range.last; d >= range.first; d--) if (!idx.has(d)) todo.push(d);
  const worker = async () => {
    while (todo.length && gen === generation) {
      const d = todo.shift();
      try { const r = await api.day(d); idx.set(d, summarise(r)); emit(); } catch { /* skip */ }
      if (idx.size % 20 === 0) save();
    }
  };
  await Promise.all([worker(), worker(), worker()]);
  save(); running = false; scannedTo = range.last;
}
export function reset() { generation++; idx.clear(); running = false; scannedTo = 0; try { sessionStorage.removeItem(KEY); } catch { /* ignore */ } }
