// People: the humans. Each one's queue, minutes, approvals and overrides, and how small the human
// footprint is next to the AI organisation. Individual calibration is private and never shown.
import { api } from '../api.js';
import { esc, fmt, pct, humanize, md, minutes, spark, clock, $ } from '../util.js';
import { KIND_COL } from '../catalog.js';
import * as DI from '../dayindex.js';

const ROLE_ORDER = ['leader', 'validator', 'end_node', 'auditor'];
const ROLE_LABEL = { leader: 'Leadership — framing and accountability', validator: 'Validators — the Domain Expert Gate', end_node: 'End nodes — taste, voice, signatures', auditor: 'Auditors' };

export async function render(el, ctx, rt) {
  const id = rt.parts[0] && rt.parts[0] !== 'day' ? rt.parts[0] : null;
  const S = await api.state().catch(() => ({ queue: [] }));
  if (!rt.isCurrent()) return;
  return id ? renderPerson(el, ctx, rt, id, S) : renderAll(el, ctx, rt, S);
}

function aggregate(ctx) {
  const R = ctx.days, per = {}; let days = 0;
  for (let d = R.first; d <= R.last; d++) {
    const s = DI.get(d); if (!s) continue; days++;
    for (const [p, m] of Object.entries(s.hl || {})) { const x = (per[p] ||= { min: 0, v: {}, series: {} }); x.min += m; x.series[d] = m; }
    for (const [p, v] of Object.entries(s.hv || {})) { const x = (per[p] ||= { min: 0, v: {}, series: {} }); for (const [k, n] of Object.entries(v)) x.v[k] = (x.v[k] || 0) + n; }
  }
  return { per, days };
}

function renderAll(el, ctx, rt, S) {
  const people = ctx.meta.people || [];
  const mods = (ctx.meta.modules || []).length;
  const R = ctx.days;
  el.innerHTML = `<div class="wrap"><div class="ph"><div><div class="kicker">People</div><h1>${people.length} humans, ${mods} AI modules</h1>
    <p class="sub">The book’s three human tasks — framing, judgment and accountability — sit with a handful of people. In this simulation they are played by LLM stand-ins unless a real human steps in through <a href="#/run">Run → human input</a>. ${ctx.help('humans')}</p></div></div>
    <section class="panel" data-tour="footprint2"><div class="bigs" id="pbig"></div><p class="tiny faint" id="pnote"></p></section>
    <div id="groups"></div></div>`;
  const draw = () => {
    const A = aggregate(ctx);
    const tot = Object.values(A.per).reduce((a, x) => a + x.min, 0);
    const acts = Object.values(A.per).reduce((a, x) => a + Object.entries(x.v).filter(([k]) => !['real', 'override'].includes(k)).reduce((s, [, n]) => s + n, 0), 0);
    const real = Object.values(A.per).reduce((a, x) => a + (x.v.real || 0), 0);
    const cap = people.reduce((a, p) => a + (p.weekly_capacity_min || 0), 0) / 7;
    $('#pbig', el).innerHTML = `<div class="big"><span>Human time / day</span><b>${minutes(tot / Math.max(1, A.days))}</b><small>of ${minutes(cap)} capacity (${pct(tot / Math.max(1, A.days) / Math.max(1, cap))})</small></div>
      <div class="big"><span>Human acts</span><b>${fmt(acts)}</b><small>approvals, reviews, framing…</small></div>
      <div class="big"><span>By a real human</span><b style="color:${real ? 'var(--ok)' : 'var(--danger)'}">${fmt(real)}</b><small>${pct(real / Math.max(1, acts), 2)} of all human acts</small></div>
      <div class="big"><span>Open queue</span><b>${(S.queue || []).length}</b><small>items waiting</small></div>`;
    $('#pnote', el).textContent = A.days < R.count ? `Totals over ${A.days} of ${R.count} indexed days (indexing in the background)…` : `Totals over all ${R.count} days.`;
    const byRole = ROLE_ORDER.map(r => [r, people.filter(p => p.role === r)]).filter(([, l]) => l.length);
    $('#groups', el).innerHTML = byRole.map(([r, list]) => `<section class="sect"><h2 style="font-size:17px"><span class="dot" style="background:${KIND_COL[r]}"></span> ${esc(ROLE_LABEL[r] || r)}</h2>
      <div class="tscroll"><table class="t"><thead><tr><th>person</th><th class="num">queue</th><th class="num">min/day</th><th>minutes over time</th><th class="num">approve</th><th class="num">reject</th><th class="num">amend</th><th class="num">overrides</th><th class="num">real-human acts</th></tr></thead><tbody>
      ${list.map(p => { const x = A.per[p.id] || { min: 0, v: {}, series: {} }; const q = (S.queue || []).filter(i => i.person === p.id).length; const ser = []; for (let d = R.first; d <= R.last; d++) ser.push(x.series[d] ?? (DI.get(d) ? 0 : null));
        return `<tr class="click" data-id="${esc(p.id)}"><td><b>${esc(p.name)}</b><div class="tiny muted">${esc(p.title || '')}</div></td><td class="num">${q || ''}</td><td class="num">${fmt(x.min / Math.max(1, A.days), 1)}</td><td style="min-width:120px">${spark(ser, { h: 22, color: KIND_COL[p.role] || 'var(--gold)' })}</td><td class="num">${x.v.approve || ''}</td><td class="num">${x.v.reject || ''}</td><td class="num">${x.v.amend || ''}</td><td class="num">${x.v.override || ''}</td><td class="num" style="color:${x.v.real ? 'var(--ok)' : 'var(--faint)'}">${x.v.real || 0}</td></tr>`; }).join('')}
      </tbody></table></div></section>`).join('');
    el.querySelectorAll('tr.click').forEach(tr => tr.onclick = () => { location.hash = ctx.link.person(tr.dataset.id, ctx.day); });
  };
  draw();
  const unsub = DI.subscribe(draw);
  DI.scan(R);
  return unsub;
}

async function renderPerson(el, ctx, rt, id, S) {
  const p = (ctx.meta.people || []).find(x => x.id === id);
  if (!p) { el.innerHTML = `<div class="wrap err">Unknown person ${esc(id)}</div>`; return; }
  const R = ctx.days, day = rt.day || ctx.day || R.last;
  const rec = await api.day(day);
  if (!rt.isCurrent()) return;
  DI.put(day, rec);
  const acts = rec.human_actions.filter(h => h.person === id);
  const q = (S.queue || []).filter(i => i.person === id);
  const accountable = (ctx.meta.classes || []).filter(c => c.accountable === id);
  const L = ctx.link;
  el.innerHTML = `<div class="wrap"><div class="ph"><div><div class="kicker"><a href="#/people">People</a> · ${esc(humanize(p.role))} · day ${day} · ${esc(ctx.dateOf(day))}</div>
      <h1><span class="dot" style="background:${KIND_COL[p.role]};width:12px;height:12px"></span> ${esc(p.name)}</h1><p class="sub">${esc(p.title || '')} · capacity ${minutes(p.weekly_capacity_min)}/week · LLM stand-in tier <code>${esc(p.llm_tier)}</code>${p.shadow_group ? ' · shadow group' : ''}</p></div>
      <div class="tools"><a class="btn" href="${L.person(id, day - 1)}">‹</a><span class="btn" style="cursor:default">Day ${day}</span><a class="btn" href="${L.person(id, day + 1)}">›</a></div></div>
    <div class="grid g3">
      <section class="panel"><h3>Today</h3>${ctx.help('humans')}<p class="small">${minutes(rec.human_load?.[id] || 0)} of work · ${rec.queue_open?.[id] ?? 0} open at end of day</p>
        <ul class="list">${acts.map(h => `<li><b>${esc(h.verdict || h.kind)}</b> <span class="faint">${esc(humanize(h.kind))}</span> <span class="${h.source === 'real_human' ? 'src-real' : 'src-standin'}">${h.source === 'real_human' ? 'REAL HUMAN' : 'stand-in'}</span>${h.manual_check ? ' <span class="chip" title="Read raw data before the AI summary (glass-cockpit counter)">manual check</span>' : ''}${h.decision ? ` · <a href="${L.decision(h.decision)}">${esc(h.decision)}</a>` : ''}<div class="meta">${clock(h.ts)} · ${minutes(h.minutes_spent)} · ${esc(h.reason || '')}</div></li>`).join('') || '<li class="empty">No actions this day.</li>'}</ul></section>
      <section class="panel"><h3>Queue now</h3>${ctx.help('humans')}<ul class="list">${q.map(i => `<li><span class="chip">${esc(humanize(i.kind))}</span>${i.blocking ? ' <span class="chip" style="color:var(--danger)">blocking</span>' : ''} ${esc(i.subject)}<div class="meta">since day ${i.created_day}${i.decision ? ` · <a href="${L.decision(i.decision)}">${esc(i.decision)}</a>` : ''}</div></li>`).join('') || '<li class="empty">Empty.</li>'}</ul></section>
      <section class="panel"><h3>Accountable for</h3>${ctx.help('autonomy')}<ul class="list">${accountable.map(c => `<li><a href="#/ledger?class=${encodeURIComponent(c.id)}">${esc(c.id)}</a> <span class="faint tiny">${esc(c.path)} · ${esc(humanize(c.reversibility))}</span></li>`).join('') || '<li class="empty">No decision classes.</li>'}</ul>
        ${p.gate_rules?.length ? `<h4 class="tiny muted">GATE RULES THEY WROTE</h4><ul class="list mono tiny">${p.gate_rules.map(g => `<li>${esc(g.classes.join(', '))}<br><span style="color:var(--danger)">red if ${esc(g.red_if || '–')}</span><br><span style="color:#ffc94a">yellow if ${esc(g.yellow_if || '–')}</span></li>`).join('')}</ul>` : ''}</section>
    </div>
    <div class="grid g2" style="margin-top:14px">
      <section class="panel"><h3>Persona</h3><div class="small">${md(p.persona)}</div>
        <dl class="kv small"><dt>expertise</dt><dd>${(p.expertise || []).map(esc).join(', ')}</dd><dt>bias profile</dt><dd>${Object.entries(p.bias || {}).map(([k, v]) => `${esc(humanize(k))} ${fmt(v, 2)}`).join(' · ')}</dd></dl>
        <p class="tiny faint">The bias profile drives the stand-in so pathogens can genuinely emerge. Personal calibration is private to the person and not shown.</p></section>
      <section class="panel"><h3>How they judge</h3><div class="small">${md(p.judgment)}</div></section>
    </div></div>`;
}
