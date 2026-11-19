// API client for the ota-server contract (docs/DATA.md). Everything is cached per session.
// Base: `?api=<url>` overrides; otherwise `../api/` relative to the viewer (/ota/sim/ → /ota/api/).

function detectBase() {
  const q = new URLSearchParams(location.search).get('api');
  if (q) return q.replace(/\/?$/, '/');
  return new URL('../api/', location.href.replace(/[#?].*$/, '')).href;
}
export const BASE = detectBase();

const cache = new Map();
const LRU_DAYS = 60; // day files can be large; keep a bounded number in memory
const dayKeys = [];

async function getJSON(path, opts = {}) {
  const r = await fetch(BASE + path, { headers: opts.headers || {}, cache: opts.fresh ? 'no-store' : 'default' });
  let body = null;
  const txt = await r.text();
  try { body = txt ? JSON.parse(txt) : null; } catch { body = txt; }
  if (!r.ok) {
    const e = new Error((body && body.error) || `${r.status} ${r.statusText}`);
    e.status = r.status; throw e;
  }
  return body;
}

function cached(key, fn) {
  if (!cache.has(key)) {
    const p = fn().catch(e => { cache.delete(key); throw e; });
    cache.set(key, p);
  }
  return cache.get(key);
}

export const api = {
  meta: () => cached('meta', () => getJSON('meta')),
  state: () => cached('state', () => getJSON('state')),
  days: () => cached('days', () => getJSON('days')),
  series: (metrics) => cached('series:' + (metrics || ''), () => getJSON('series' + (metrics ? '?metrics=' + encodeURIComponent(metrics) : ''))),
  day(d) {
    const k = 'day:' + d;
    if (!cache.has(k)) { dayKeys.push(k); while (dayKeys.length > LRU_DAYS) cache.delete(dayKeys.shift()); }
    return cached(k, () => getJSON('day/' + d));
  },
  // Module view of a day; null when the module had nothing that day (404).
  dayModule: (d, m) => cached(`dm:${d}:${m}`, () => getJSON(`day/${d}/module/${encodeURIComponent(m)}`).catch(e => { if (e.status === 404) return null; throw e; })),
  decision: (id) => cached('dec:' + id, () => getJSON('decision/' + encodeURIComponent(id))),
  ledger(params) {
    const qs = new URLSearchParams(Object.entries(params || {}).filter(([, v]) => v !== '' && v != null)).toString();
    return cached('ledger:' + qs, () => getJSON('ledger' + (qs ? '?' + qs : '')));
  },
  runStatus: () => getJSON('run/status', { fresh: true }),
  async post(path, body, token) {
    const r = await fetch(BASE + path, { method: 'POST', headers: { 'content-type': 'application/json', 'X-OTA-Admin': token || '' }, body: JSON.stringify(body) });
    const txt = await r.text(); let j; try { j = JSON.parse(txt); } catch { j = { error: txt }; }
    if (!r.ok) { const e = new Error(j.error || r.statusText); e.status = r.status; throw e; }
    return j;
  },
  // Forget everything that can change after a run (keeps nothing; cheap to refetch).
  invalidate() { cache.clear(); dayKeys.length = 0; }
};
