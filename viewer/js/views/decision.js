// Decision: one ledger entry in full (7 fields highlighted, switches, pre-mortem, challenges,
// gate, approvals, actions, sealed outcome + Brier). Ledger: search and filter.
import { api } from '../api.js';
import { esc, fmt, pct, humanize, md, clock, minutes, debounce, $ } from '../util.js';
import { RUNG_SHORT, STATUS_COL, GATE_COL } from '../catalog.js';
import { rungBar } from './module.js';

export async function render(el, ctx, rt) {
  if (rt.view === 'ledger' || !rt.parts[0]) return renderLedger(el, ctx, rt);
  const id = rt.parts[0];
  const r = await api.decision(id);
  const d = r.decision, L = ctx.link;
  const cls = (ctx.meta.classes || []).find(c => c.id === d.class);
  const f = (n, label, val, vocabId) => `<div class="f${val ? '' : ' missing'}"><div class="n"><i>${n}</i>${label}</div><div class="v">${val || '<span class="faint">missing</span>'}</div></div>`;
  const infoLinks = (d.information || []).map(x => /^D-/.test(x) ? `<a class="mono" href="${L.decision(x)}">${esc(x)}</a>` : `<span class="mono">${esc(x)}</span>`).join(' ');
  const steps = ['proposed', 'approved', 'executing', 'sealed'];
  const si = steps.indexOf(d.status);
  const side = ['rejected', 'reverted', 'blocked', 'deferred'].includes(d.status);
  const forecast = r.forecasts[0];
  el.innerHTML = `<div class="wrap">
    <div class="ph"><div><div class="kicker"><a href="#/ledger">Decision Ledger</a> · ${esc(d.id)} · day ${d.when} · ${esc(ctx.dateOf(d.when))}</div>
      <h1 style="font-size:clamp(20px,2.6vw,28px)">${esc(d.what)}</h1>
      <p class="sub"><span class="st" style="color:${STATUS_COL[d.status] || 'inherit'}">${esc(d.status)}</span> · <span class="tier t-${d.tier}">${d.tier}</span> · ${ctx.nodeChip(d.module, d.when)} · class <a href="#/ledger?class=${encodeURIComponent(d.class)}">${esc(d.class)}</a> · accountable ${ctx.nodeChip(d.accountable, d.when)}</p>
      <div class="lifecycle">${steps.map((s, i) => `<span class="${i === si ? 'on' : i < si ? 'past' : ''}">${s}</span>`).join('')}${side ? `<span class="on" style="background:${STATUS_COL[d.status]};border-color:${STATUS_COL[d.status]}">${esc(d.status)}</span>` : ''}</div></div>
      <div class="tools"><a class="btn" href="${L.module(d.module, d.when)}">Module that day →</a><a class="btn" href="#/birdseye/day/${d.when}">Birdseye →</a></div></div>

    <section class="panel" data-tour="seven"><header><h3>The seven fields</h3>${ctx.help('ledger-fields')}<span class="r tiny faint">${d.committed_day ? `committed day ${d.committed_day} ${clock(d.committed_ts)}` : 'not committed'}</span></header>
      <div class="seven">
        ${f(1, 'What', esc(d.what))}
        ${f(2, 'When', `day ${d.when} <span class="faint">(${esc(ctx.dateOf(d.when))})</span>`)}
        ${f(3, 'Who decided', ctx.nodeChip(d.who, d.when) + (d.who !== d.module ? ` <span class="tiny faint">proposed by ${esc(ctx.name(d.module))}</span>` : ''))}
        ${f(4, 'Information', infoLinks)}
        ${f(5, 'Assumptions', (d.assumptions || []).map(a => `<a class="mono" href="#/health?assumption=${encodeURIComponent(a)}">${esc(a)}</a>`).join(' '))}
        ${f(6, 'Expected outcome', esc(d.expected))}
        ${f(7, 'Decision switches', d.switches ? `<div class="mono tiny">confirm if <b style="color:var(--ok)">${esc(d.switches.confirm_if)}</b><br>stop if <b style="color:var(--danger)">${esc(d.switches.stop_if)}</b><br>check by day ${d.switches.check_by}</div>` : '')}
      </div></section>

    <div class="grid g2" style="margin-top:14px">
      <section class="panel"><h3>Reasoning</h3>${ctx.help('decisions')}
        <div class="small">${md(d.reasoning)}</div>
        ${d.alternatives?.length ? `<p class="tiny muted">Alternatives considered: ${d.alternatives.map(a => `<code>${esc(a)}</code>`).join(' ')}</p>` : ''}
        <dl class="kv" style="margin-top:8px">
          <dt>action</dt><dd class="mono tiny">${esc(d.action?.kind)} ${esc(JSON.stringify(d.action?.params || {}))}</dd>
          <dt>class</dt><dd>${esc(d.kind)} · ${esc(d.path)} · ${esc(humanize(d.reversibility))} · ${esc(humanize(d.impact))}${cls?.human_only ? ' · <b>human only</b>' : ''}</dd>
          <dt>stakes × novelty</dt><dd>€${fmt(d.stakes_eur)} × ${fmt(d.novelty, 2)}</dd>
          <dt>confidence</dt><dd>${pct(d.confidence)}</dd>
          <dt>rung at the time</dt><dd>${rungBar(d.rung_at_time)}</dd>
          <dt>informed</dt><dd>${(d.informed || []).map(x => ctx.nodeChip(x)).join(' ') || '–'}</dd>
          ${d.commitment_point ? `<dt>commitment point</dt><dd>${esc(d.commitment_point)}</dd>` : ''}
          ${d.first_signal_day ? `<dt>decision latency</dt><dd>${(d.committed_day || d.when) - d.first_signal_day} day(s) from first signal (day ${d.first_signal_day})</dd>` : ''}
        </dl>
        ${d.action_elements && Object.values(d.action_elements).some(Boolean) ? `<h4 class="tiny muted" style="margin-top:12px">FOUR ELEMENTS OF ACTION ${ctx.help('honest')}</h4><dl class="kv small">${Object.entries(d.action_elements).map(([k, v]) => `<dt>${esc(humanize(k))}</dt><dd>${v ? esc(v) : '<span style="color:var(--danger)">missing</span>'}</dd>`).join('')}</dl>` : ''}
        ${d.notes?.length ? `<ul class="list tiny">${d.notes.map(n => `<li>${esc(n)}</li>`).join('')}</ul>` : ''}
      </section>
      <section class="panel"><h3>Outcome</h3>${ctx.help('tool-ledger')}
        ${d.outcome ? `<div class="big"><span>sealed day ${d.outcome.sealed_day}</span><b style="color:${d.outcome.success === true ? 'var(--ok)' : d.outcome.success === false ? 'var(--danger)' : 'var(--muted)'}">${d.outcome.success === true ? 'Confirmed' : d.outcome.success === false ? 'Stopped' : 'Inconclusive'}</b></div>
          <dl class="kv small" style="margin-top:8px"><dt>actual</dt><dd>${esc(d.outcome.actual)}</dd><dt>expected</dt><dd>${esc(d.outcome.expected)}</dd><dt>deviation</dt><dd>${esc(d.outcome.deviation)}</dd><dt>metrics</dt><dd class="mono tiny">${Object.entries(d.outcome.metrics || {}).map(([k, v]) => `${esc(k)}=${fmt(v)}`).join(' · ')}</dd></dl>`
          : `<p class="empty">Not sealed yet${d.switches ? ` — switches are checked on day ${d.switches.check_by}` : ''}.</p>`}
        ${r.forecasts.length ? `<h4 class="tiny muted" style="margin-top:12px">FORECASTS ${ctx.help('tool-calibration')}</h4><ul class="list">${r.forecasts.map(x => `<li>${esc(x.question)}<div class="meta">p = ${fmt(x.p, 2)} · resolves day ${x.resolves_on} · ${x.outcome == null ? (d.outcome ? 'resolved with the outcome (day ' + d.outcome.sealed_day + ')' : 'open') : x.outcome ? 'happened' : 'did not happen'}${x.brier != null ? ` · <b>Brier ${fmt(x.brier, 3)}</b>` : ''}</div></li>`).join('')}</ul>` : ''}
        ${r.actions.length || d.actions?.length ? `<h4 class="tiny muted" style="margin-top:12px">ACTIONS</h4><ul class="list">${r.actions.map(a => `<li><b>${esc(a.kind)}</b> ${esc(a.effect_summary)} <span class="faint tiny">day ${a.executed_day}</span></li>`).join('')}${(d.actions || []).filter(x => !r.actions.some(a => a.id === x)).map(x => `<li class="mono tiny">${esc(x)} (later day)</li>`).join('')}</ul>` : ''}
      </section>
    </div>

    <div class="grid g3" style="margin-top:14px">
      <section class="panel"><h3>Pre-mortem</h3>${ctx.help('tool-premortem')}
        ${r.premortem ? (r.premortem.skipped ? `<p class="empty">Skipped: ${esc(r.premortem.skip_reason)}</p>` : `<p class="tiny muted">started ${clock(r.premortem.started_ts)}${d.committed_ts != null && r.premortem.started_ts != null ? ` · ${d.committed_ts - r.premortem.started_ts} min before commitment` : ''} · ${esc(r.premortem.tier)} tier</p><ul class="list">${r.premortem.failure_modes.map(m => `<li>${esc(m.text)}<div class="meta">p ${fmt(m.p, 2)} · severity ${fmt(m.severity, 2)}</div></li>`).join('')}</ul>${r.premortem.load_bearing.length ? `<p class="tiny">load-bearing: ${r.premortem.load_bearing.map(esc).join(', ')}</p>` : ''}`) : '<p class="empty">None (below the stakes × novelty threshold, or the tool was not on yet).</p>'}
      </section>
      <section class="panel"><h3>Challenges</h3>${ctx.help('tool-adversarial')}
        ${r.challenges.length ? `<ul class="list">${r.challenges.map(c => `<li><b>${esc(c.target)}</b> · strength ${fmt(c.strength, 2)}${c.accepted === true ? ' · <span style="color:var(--ok)">accepted</span>' : c.accepted === false ? ' · rejected' : ''}<div class="small">${esc(c.argument)}</div>${c.response ? `<div class="meta">response: ${esc(c.response)}</div>` : ''}${c.overridden_by ? `<div class="meta" style="color:#ffb35c">overridden by ${esc(ctx.name(c.overridden_by))}</div>` : ''}</li>`).join('')}</ul>` : '<p class="empty">No challenge attached.</p>'}
      </section>
      <section class="panel"><h3>Gate &amp; approvals</h3>${ctx.help('tool-gate')}
        ${d.gate ? `<p><b style="color:${GATE_COL[d.gate.color]}">● ${esc(d.gate.color.toUpperCase())}</b> <span class="tiny mono">${esc(d.gate.rule)}</span></p><p class="tiny muted">${d.gate.validator ? 'validator ' + esc(ctx.name(d.gate.validator)) : ''}${d.gate.reviewed_by ? ` · reviewed by ${d.gate.reviewed_by === 'ai' ? '<b>AI (Path 2)</b>' : esc(ctx.name(d.gate.reviewed_by))} day ${d.gate.reviewed_day}` : ''}${d.gate.verdict ? ' · ' + esc(d.gate.verdict) : ''}${d.gate.recommendation ? ' · ' + esc(d.gate.recommendation) : ''}</p>` : '<p class="empty">No gate review (tool not on yet).</p>'}
        ${d.approvals.length ? `<ul class="list">${d.approvals.map(a => `<li>${ctx.nodeChip(a.by, a.day)} <b>${esc(a.verdict)}</b>${a.post_hoc ? ' <span class="chip">post-hoc review</span>' : ''} <span class="${a.source === 'real_human' ? 'src-real' : 'src-standin'}">${a.source === 'real_human' ? 'REAL HUMAN' : 'stand-in'}</span><div class="meta">day ${a.day} ${clock(a.ts)} · ${minutes(a.minutes_spent)} · ${esc(a.reason)}</div>${a.amendments ? `<div class="meta">amendments: ${esc(typeof a.amendments === 'string' ? a.amendments : JSON.stringify(a.amendments))}</div>` : ''}</li>`).join('')}</ul>` : '<p class="empty">No human approval (executed at its rung, or waiting).</p>'}
        ${d.overrides.length ? `<h4 class="tiny muted">OVERRIDES</h4><ul class="list">${d.overrides.map(o => `<li>${ctx.nodeChip(o.by)} ${esc(o.what)} <span class="${o.source === 'real_human' ? 'src-real' : 'src-standin'}">${esc(o.source)}</span><div class="meta">${esc(o.reason)}</div></li>`).join('')}</ul>` : ''}
      </section>
    </div>
    ${r.assumptions.length ? `<section class="panel" style="margin-top:14px"><h3>Assumptions it rests on</h3>${ctx.help('tool-assumptions')}<ul class="list">${r.assumptions.map(a => `<li><b class="mono">${esc(a.id)}</b> ${esc(a.text)}<div class="meta">${esc(a.status)} · confidence ${pct(a.confidence)}${a.load_bearing ? ' · load-bearing' : ''} · owner ${esc(ctx.name(a.owner))} · last confirmed day ${a.last_confirmed}</div></li>`).join('')}</ul></section>` : ''}
  </div>`;
}

async function renderLedger(el, ctx, rt) {
  const q = rt.query;
  const p = { q: q.get('q') || '', class: q.get('class') || '', module: q.get('module') || '', status: q.get('status') || '', from: q.get('from') || '', to: q.get('to') || '', limit: q.get('limit') || 200 };
  const mods = ctx.meta.modules || [];
  const statuses = ['proposed', 'approved', 'executing', 'sealed', 'rejected', 'reverted', 'blocked', 'deferred'];
  el.innerHTML = `<div class="wrap"><div class="ph"><div><div class="kicker">Decision Ledger</div><h1>Every decision, with an exit</h1><p class="sub">Each entry records what, when, who, on what information, resting on which assumptions, expecting what, and the switches that will confirm or stop it. ${ctx.help('ledger-fields')}</p></div></div>
    <section class="panel" data-tour="ledgerfilter"><div class="row">
      <input id="lq" type="search" placeholder="Search id, class, text…" value="${esc(p.q)}" style="flex:1 1 220px">
      <select id="lmod"><option value="">all modules</option>${mods.map(m => `<option value="${esc(m.id)}"${m.id === p.module ? ' selected' : ''}>${esc(m.name || m.id)}</option>`).join('')}</select>
      <select id="lst"><option value="">any status</option>${statuses.map(s => `<option${s === p.status ? ' selected' : ''}>${s}</option>`).join('')}</select>
      <input id="lcls" placeholder="class (e.g. ua.*)" value="${esc(p.class)}" style="width:150px">
      <input id="lfrom" type="number" placeholder="from day" value="${esc(p.from)}" style="width:96px"><input id="lto" type="number" placeholder="to day" value="${esc(p.to)}" style="width:96px">
    </div></section>
    <section class="panel" style="margin-top:14px"><div id="lres" class="loading">Searching…</div></section></div>`;
  const run = async () => {
    const np = { q: $('#lq', el).value.trim(), module: $('#lmod', el).value, status: $('#lst', el).value, class: $('#lcls', el).value.trim(), from: $('#lfrom', el).value, to: $('#lto', el).value };
    const qs = new URLSearchParams(Object.entries(np).filter(([, v]) => v)).toString();
    history.replaceState(null, '', '#/ledger' + (qs ? '?' + qs : ''));
    let res;
    try { res = await api.ledger({ ...np, limit: 200 }); } catch (e) { $('#lres', el).innerHTML = `<p class="err">${esc(e.message)}</p>`; return; }
    const out = res.decisions;
    $('#lres', el).innerHTML = `<p class="tiny muted">${res.count} shown of ${res.total} matching (newest first)</p><div class="tscroll"><table class="t"><thead><tr><th>id</th><th>day</th><th>what</th><th>module</th><th>tier</th><th>status</th><th>gate</th><th class="num">stakes</th><th>outcome</th></tr></thead><tbody>
      ${out.map(d => `<tr class="click" data-id="${esc(d.id)}"><td class="mono tiny">${esc(d.id)}</td><td>${d.when}</td><td>${esc(d.what)}</td><td class="small">${esc(ctx.name(d.module))}</td><td><span class="tier t-${d.tier}">${d.tier}</span></td><td><span class="st" style="color:${STATUS_COL[d.status] || 'inherit'}">${esc(d.status)}</span></td><td>${d.gate ? `<span style="color:${GATE_COL[d.gate.color]}">●</span>` : ''}</td><td class="num">€${fmt(d.stakes_eur)}</td><td>${d.outcome ? (d.outcome.success === true ? '<span style="color:var(--ok)">✓</span>' : d.outcome.success === false ? '<span style="color:var(--danger)">✗</span>' : '?') : ''}</td></tr>`).join('')}
      </tbody></table></div>${out.length ? '' : '<p class="empty">No decisions match.</p>'}`;
    el.querySelectorAll('tr.click').forEach(tr => tr.onclick = () => { location.hash = ctx.link.decision(tr.dataset.id); });
  };
  const deb = debounce(run, 300);
  el.querySelectorAll('input,select').forEach(i => i.addEventListener('input', deb));
  await run();
}
