// Run: advance the simulation N days (admin token), live progress, cost and model per tier,
// and the real-human input form (framing, verdicts) that replaces a stand-in for one act.
import { api } from '../api.js';
import { reloadMeta } from '../app.js';
import { esc, fmt, usd, store, humanize, $ } from '../util.js';
import { TIER_COL } from '../catalog.js';
import * as DI from '../dayindex.js';

const TOKEN = 'ota.adminToken';

export async function render(el, ctx) {
  const M = ctx.meta, R = ctx.days;
  const tiers = Object.entries(M.llm?.tiers || {});
  el.innerHTML = `<div class="wrap"><div class="ph"><div><div class="kicker">Run control</div><h1>Advance the company</h1>
    <p class="sub">History days are deterministic (scripted). New days use live models when the server has a key (<b>${M.live_llm_available ? 'available' : 'not available — scripted responder'}</b>), under a daily budget of ${usd(M.llm?.daily_budget_usd)}. ${M.autopilot ? `Autopilot advances ${M.autopilot.days} day(s) every night at ${String(M.autopilot.hour).padStart(2, '0')}:00 ${esc(M.autopilot.timezone)}.` : 'Autopilot is off.'} ${ctx.help('run')}</p></div></div>
    <div class="grid g2">
      <section class="panel" data-tour="run"><h3>Run</h3>
        <div id="status" class="small">…</div>
        <div class="row" style="margin-top:12px"><label class="small muted">days <input id="ndays" type="number" min="1" max="31" value="1" style="width:70px"></label>
          <button class="btn gold" id="go" ${M.admin_enabled ? '' : 'disabled'}>Advance</button><button class="btn" id="tok">${store.get(TOKEN) ? 'Change token' : 'Set admin token'}</button></div>
        ${M.admin_enabled ? '' : '<p class="tiny faint">Admin is disabled on this server (no OTA_ADMIN_TOKEN): runs and human input are read-only.</p>'}
        <p class="tiny faint">The token is kept only in this browser (localStorage). Strangers cannot spend tokens: every POST needs it.</p>
      </section>
      <section class="panel"><h3>Models per tier</h3>${ctx.help('llm')}
        <div class="tscroll"><table class="t small"><thead><tr><th>tier</th><th>model</th><th>provider</th><th class="num">$/M in</th><th class="num">$/M out</th></tr></thead><tbody>
        ${tiers.map(([t, c]) => `<tr><td><span class="tier t-${esc(t)}" style="${TIER_COL[t] ? '' : 'background:#7fb2c9'}">${esc(t)}</span></td><td class="mono">${esc(c.model)}</td><td>${esc(c.provider)}</td><td class="num">${fmt(c.price_in)}</td><td class="num">${fmt(c.price_out)}</td></tr>`).join('')}
        </tbody></table></div><p class="tiny muted">Fallback order when the budget runs out: ${(M.llm?.fallback || []).map(esc).join(' → ')} → rules.</p>
        <h4 class="tiny muted" style="margin-top:12px">COST PER TIER, LAST 14 DAYS</h4><div id="cost" class="small">loading…</div></section>
    </div>
    <section class="panel" style="margin-top:14px" data-tour="human"><h3>Human input — act as a real human</h3>${ctx.help('honest')}
      <p class="small muted">Everything a person does here is recorded with <b class="src-real">source: real_human</b> and applied at the start of the next simulated day, before the stand-ins act. This is the mitigation for the book’s biggest risk in this setup: LLMs doing the human framing.</p>
      <div class="grid g2">
        <div><label class="small muted">as person<br><select id="hperson" style="width:100%">${(M.people || []).map(p => `<option value="${esc(p.id)}">${esc(p.name)} — ${esc(p.title || p.role)}</option>`).join('')}</select></label>
          <div class="seg" id="hmode" style="margin-top:10px"><button class="on" data-m="framing">Framing</button><button data-m="item">Queue item</button><button data-m="decision">Decision</button></div>
          <div id="hfields" style="margin-top:10px"></div></div>
        <div><label class="small muted">message / reason<br><textarea id="htext" placeholder="e.g. Focus on retention, not installs, until the season ends."></textarea></label>
          <button class="btn gold" id="hsend" style="margin-top:8px" ${M.admin_enabled ? '' : 'disabled'}>Queue for the next day</button><div id="hres" class="small" style="margin-top:8px"></div></div>
      </div></section></div>`;

  const S = await api.state().catch(() => ({ queue: [] }));
  let mode = 'framing';
  const fields = () => {
    const person = $('#hperson', el).value;
    const items = (S.queue || []).filter(i => i.person === person);
    $('#hfields', el).innerHTML = mode === 'framing'
      ? `<label class="small muted">to (empty = all modules)<br><select id="hto" multiple size="6" style="width:100%">${(M.modules || []).map(m => `<option value="${esc(m.id)}">${esc(m.name || m.id)}</option>`).join('')}</select></label>`
      : mode === 'item'
        ? (items.length ? `<label class="small muted">open item<br><select id="hitem" style="width:100%">${items.map(i => `<option value="${esc(i.id)}">${esc(humanize(i.kind))}: ${esc(i.subject)}</option>`).join('')}</select></label>` : '<p class="empty">This person has no open queue items.</p>') + verdictSel()
        : `<label class="small muted">decision id<br><input id="hdec" placeholder="D-0100-0002" style="width:100%"></label>` + verdictSel();
  };
  const verdictSel = () => `<label class="small muted" style="display:block;margin-top:8px">verdict<br><select id="hverdict"><option>approve</option><option>reject</option><option>amend</option><option>defer</option></select></label>`;
  $('#hperson', el).onchange = fields;
  el.querySelectorAll('#hmode button').forEach(b => b.onclick = () => { mode = b.dataset.m; el.querySelectorAll('#hmode button').forEach(x => x.classList.toggle('on', x === b)); fields(); });
  fields();

  const token = () => { let t = store.get(TOKEN); if (!t) { t = prompt('Admin token (X-OTA-Admin). Kept in this browser only.'); if (t) store.set(TOKEN, t.trim()); } return store.get(TOKEN); };
  $('#tok', el).onclick = () => { const t = prompt('Admin token (empty to forget)', ''); store.set(TOKEN, t ? t.trim() : null); $('#tok', el).textContent = t ? 'Change token' : 'Set admin token'; };
  const authErr = e => e.status === 401 ? (store.set(TOKEN, null), 'Wrong admin token — it has been forgotten; try again.') : e.status === 403 ? 'Admin is disabled on this server.' : e.status === 409 ? 'A run is already in progress.' : e.message;

  $('#hsend', el).onclick = async () => {
    const t = token(); if (!t) return;
    const body = { person: $('#hperson', el).value, reason: $('#htext', el).value.trim() };
    if (mode === 'framing') { body.framing = body.reason; delete body.reason; const to = [...($('#hto', el)?.selectedOptions || [])].map(o => o.value); if (to.length) body.to = to; if (!body.framing) return void ($('#hres', el).textContent = 'Write the framing first.'); }
    else if (mode === 'item') { const it = $('#hitem', el)?.value; if (!it) return; body.item = it; body.verdict = $('#hverdict', el).value; }
    else { body.decision = $('#hdec', el).value.trim(); body.verdict = $('#hverdict', el).value; if (!body.decision) return; }
    try { const r = await api.post('human-input', body, t); $('#hres', el).innerHTML = `<span style="color:var(--ok)">Queued.</span> Applies on day ${r.applies_on_day}.`; }
    catch (e) { $('#hres', el).innerHTML = `<span class="err">${esc(authErr(e))}</span>`; }
  };

  let poll = null, wasRunning = false;
  const drawStatus = async () => {
    let s; try { s = await api.runStatus(); } catch (e) { $('#status', el).innerHTML = `<span class="err">${esc(e.message)}</span>`; return; }
    const done = s.done_days || 0, req = s.requested_days || 0;
    $('#status', el).innerHTML = `<dl class="kv"><dt>state</dt><dd>${s.running ? '<b style="color:var(--ok)">running</b>' : 'idle'} ${s.trigger ? '· ' + esc(s.trigger) : ''}</dd>
      <dt>progress</dt><dd>${req ? `${done}/${req} days` : '–'} <div class="bar" style="margin-top:4px"><i style="width:${req ? done / req * 100 : 0}%"></i></div></dd>
      <dt>current day</dt><dd>${s.current_day ?? R.last}</dd><dt>cost this run</dt><dd>${usd(s.cost_usd)} · ${esc(s.llm_mode || M.llm_mode)}</dd>
      <dt>started</dt><dd>${esc(s.started_at || '–')}</dd><dt>finished</dt><dd>${esc(s.finished_at || '–')}</dd>
      ${s.errors?.length ? `<dt>errors</dt><dd class="err">${s.errors.map(esc).join('<br>')}</dd>` : ''}</dl>`;
    $('#go', el).disabled = !M.admin_enabled || s.running;
    ctx.runLive = s.running;
    if (wasRunning && !s.running) { DI.reset(); await reloadMeta(); drawCost(); }
    wasRunning = s.running;
    clearTimeout(poll); poll = setTimeout(drawStatus, s.running ? 1500 : 10000);
  };
  $('#go', el).onclick = async () => {
    const t = token(); if (!t) return;
    const n = Math.max(1, Math.min(31, +$('#ndays', el).value || 1));
    try { await api.post('run', { days: n }, t); wasRunning = true; drawStatus(); }
    catch (e) { $('#status', el).insertAdjacentHTML('afterbegin', `<p class="err">${esc(authErr(e))}</p>`); }
  };

  async function drawCost() {
    const last = ctx.days.last, from = Math.max(ctx.days.first, last - 13);
    const agg = {}; let total = 0;
    const recs = await Promise.all(Array.from({ length: last - from + 1 }, (_, k) => api.day(from + k).catch(() => null)));
    for (const r of recs) for (const c of r?.llm_calls || []) { const k = c.tier + ' · ' + c.model; const a = (agg[k] ||= { tier: c.tier, n: 0, usd: 0, tok: 0, fb: 0 }); a.n++; a.usd += c.cost_usd || 0; a.tok += (c.tokens_in || 0) + (c.tokens_out || 0); if (c.fallback) a.fb++; total += c.cost_usd || 0; }
    const mx = Math.max(1e-9, ...Object.values(agg).map(a => a.usd));
    $('#cost', el).innerHTML = Object.keys(agg).length ? Object.entries(agg).sort((a, b) => b[1].usd - a[1].usd).map(([k, a]) => `<div class="hbar"><span class="l" title="${esc(k)}">${esc(k)}</span><span class="b"><i style="width:${a.usd / mx * 100}%;background:${TIER_COL[a.tier] || '#7fb2c9'}"></i></span><span class="v">${usd(a.usd)}</span></div><div class="tiny faint" style="margin:-2px 0 4px 138px">${a.n} calls · ${fmt(a.tok)} tokens${a.fb ? ` · ${a.fb} fallbacks` : ''}</div>`).join('') + `<p class="tiny muted">Total ${usd(total)} over days ${from}–${last}.</p>` : '<p class="empty">No LLM calls in the last 14 days.</p>';
  }
  drawStatus(); drawCost();
  return () => clearTimeout(poll);
}
