// Company-agnostic headline KPIs: prefer a company total (co_*), else sum per-game metrics by name.
import { api } from './api.js';

const RULES = [
  { key: 'revenue', label: 'Revenue / day', unit: 'EUR', total: /^(co_|company_)?revenue(_daily)?$|^co_revenue/, part: /revenue_daily$|(^|_)revenue$/, color: '#f1d27a' },
  { key: 'dau', label: 'DAU', total: /^(co_|company_)dau$/, part: /(^|_)dau$/, color: '#8cc8ff' },
  { key: 'org', label: 'Installs organic', total: /^(co_)?installs_organic$/, part: /installs_organic$/, color: '#5fd38d' },
  { key: 'paid', label: 'Installs paid', total: /^(co_)?installs_paid$/, part: /installs_paid$/, color: '#ff9a3d' }
];

export async function headline() {
  const s = await api.series();
  const cols = Object.keys(s.series || {});
  const out = { days: s.days || [], dates: s.dates || [], rows: [] };
  for (const r of RULES) {
    let names = cols.filter(c => r.total.test(c));
    if (!names.length) names = cols.filter(c => r.part.test(c) && !c.startsWith('ota.'));
    if (!names.length && r.key === 'org') names = cols.filter(c => c === 'installs');
    if (!names.length) continue;
    const vals = out.days.map((_, i) => { let t = null; for (const n of names) { const v = s.series[n][i]; if (v != null) t = (t || 0) + v; } return t; });
    out.rows.push({ ...r, names, vals });
  }
  const ota = k => s.series['ota.' + k] || out.days.map(() => null);
  out.ota = { decisions: ota('decisions'), human_minutes: ota('human_minutes'), llm_cost_usd: ota('llm_cost_usd'), real_human_share: ota('real_human_share'),
    alerts_suppressed: ota('alerts_suppressed'), alerts_routed: ota('alerts_routed'), brier_30d: ota('brier_30d'), external_signal_share: ota('external_signal_share'),
    ledger_completeness: ota('ledger_completeness'), decision_latency_days: ota('decision_latency_days'), loop_speed_days: ota('loop_speed_days'), hops_to_decider: ota('hops_to_decider'), learning_rate: ota('learning_rate') };
  out.raw = s;
  return out;
}
