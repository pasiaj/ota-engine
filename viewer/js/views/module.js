// Module focus: one module on one day — inputs | think trace | outputs, a 90-day activity strip,
// and its decision classes with their autonomy rungs. ←/→ move one day.
import { api } from '../api.js';
import { esc, fmt, usd, humanize, md, store, $ } from '../util.js';
import { TIER_COL, TIERS, RUNG_SHORT, RUNGS, KIND_COL, PHASE_COL, NODE_TOOL, TOOLS } from '../catalog.js';
import * as DI from '../dayindex.js';

export async function render(el, ctx, rt) {
  const id = rt.parts[0];
  const mods = ctx.meta.modules || [];
  if (!id) return renderIndex(el, ctx);
  const n = ctx.node(id);
  if (n?.isPerson) { location.replace(ctx.link.person(id, rt.day)); return; }
  const m = mods.find(x => x.id === id);
  ctx.lastModule = id; store.set('ota.lastModule', id);
  const R = ctx.days;
  let day = rt.day || ctx.day || R.last;
  const [dm, state] = await Promise.all([api.dayModule(day, id), api.state().catch(() => null)]);
  if (!rt.isCurrent()) return;
  const L = ctx.link;
  const name = m?.name || n?.name || id;
  const toolId = m?.tool || NODE_TOOL[id];
  const kind = m ? (m.tool ? 'toolkit' : m.kind) : (n?.kind || 'node');
  const classes = (ctx.meta.classes || []).filter(c => c.module === id);
  const rungs = state?.autonomy?.rungs || {};
  const calib = state?.calibration?.by_class || {};

  const picker = `<div class="picker"><select id="mpick" aria-label="Pick a module or person">
    ${groupBy(mods, x => x.tool ? 'OTA toolkit' : (x.org || 'other')).map(([g, list]) => `<optgroup label="${esc(g)}">${list.map(x => `<option value="${esc(x.id)}"${x.id === id ? ' selected' : ''}>${esc(x.name || x.id)}</option>`).join('')}</optgroup>`).join('')}
    <optgroup label="People">${(ctx.meta.people || []).map(p => `<option value="${esc(p.id)}">${esc(p.name)} — ${esc(p.title || p.role)}</option>`).join('')}</optgroup></select>
    <a class="btn" href="${L.module(id, day - 1)}" title="Previous day (←)">‹</a><span class="btn" style="cursor:default">Day ${day}</span><a class="btn" href="${L.module(id, day + 1)}" title="Next day (→)">›</a></div>`;

  el.innerHTML = `<div class="wrap">
    <div class="ph"><div><div class="kicker">Module focus · ${esc(humanize(kind))}${m?.org ? ' · ' + esc(m.org) : ''} · day ${day} · ${esc(ctx.dateOf(day))}</div>
      <h1><span class="dot" style="background:${KIND_COL[kind] || '#888'};width:12px;height:12px;vertical-align:middle"></span> ${esc(name)}</h1>
      <p class="sub">${m ? `${(m.phases || []).map(p => `<b style="color:${PHASE_COL[p]}">${p}</b>`).join(' · ')} · ${esc(m.cadence)} · accountable ${ctx.nodeChip(m.accountable, day)}${m.evolves_from ? ` · evolves from <i>${esc(m.evolves_from)}</i>` : ''}${toolId && TOOLS[toolId] ? ` · <a href="#/docs/tool-${toolId}">about this tool</a>` : ''}` : 'Engine toolkit step (pseudo-node).'}</p></div>
      <div class="tools">${picker}</div></div>
    <section class="panel" data-tour="strip"><header><h3>Activity, 90 days</h3><span class="r tiny faint" id="stripnote"></span>${ctx.help('module-strip')}</header><div class="strip" id="strip"></div>
      <div class="legend" style="margin-top:6px">${TIERS.map(t => `<span><i class="dot" style="background:${TIER_COL[t]}"></i>${t}</span>`).join('')}<span>height = decisions + messages</span><span class="faint">click a bar to jump</span></div></section>
    <div class="cols3" style="margin-top:14px" data-tour="cols">
      <section class="panel"><div class="colh" style="color:var(--obs)">① Inputs · observe ${ctx.help('module-inputs')}</div><div id="cin"></div></section>
      <section class="panel"><div class="colh" style="color:var(--think)">② Think ${ctx.help('module-think')}</div><div id="cthink"></div></section>
      <section class="panel"><div class="colh" style="color:var(--act)">③ Outputs · act ${ctx.help('module-outputs')}</div><div id="cout"></div></section>
    </div>
    ${classes.length ? `<section class="panel" style="margin-top:14px"><header><h3>Decision classes &amp; autonomy</h3>${ctx.help('autonomy')}</header><div class="tscroll"><table class="t"><thead><tr><th>class</th><th>path</th><th>reversibility</th><th>impact</th><th class="num">stakes</th><th>rung now</th><th>default</th><th>accountable</th><th class="num">AI Brier (n)</th></tr></thead><tbody>
      ${classes.map(c => { const now = rungs[c.id] || c.default_rung; const p = calib[c.id]; return `<tr class="click" data-href="#/ledger?class=${encodeURIComponent(c.id)}"><td><b>${esc(c.id)}</b>${c.human_only ? ' <span class="chip">human only</span>' : ''}<div class="tiny faint">${esc(c.forecast || '')}</div></td><td>${esc(c.path)}</td><td>${esc(humanize(c.reversibility))}</td><td>${esc(humanize(c.impact))}</td><td class="num">€${fmt(c.stakes_eur)}</td><td>${rungBar(now)} ${now !== c.default_rung ? '<b style="color:var(--gold-2)">changed</b>' : ''}</td><td class="faint">${esc(RUNG_SHORT[c.default_rung] || c.default_rung)}</td><td>${ctx.nodeChip(c.accountable)}</td><td class="num">${p && p.n ? fmt(p.brier, 3) + ' (' + p.n + ')' : '–'}</td></tr>`; }).join('')}
      </tbody></table></div>
      <details style="margin-top:8px"><summary>Decision tables</summary>${classes.map(c => tableHTML(c)).join('')}</details></section>` : ''}
    ${m?.description ? `<section class="panel" style="margin-top:14px"><details><summary>MODULE.md — mission and operating notes</summary><div class="small">${md(m.description)}</div></details></section>` : ''}
  </div>`;
  el.querySelectorAll('tr.click').forEach(tr => tr.onclick = () => { location.hash = tr.dataset.href; });
  $('#mpick', el).onchange = e => { location.hash = ctx.link.node(e.target.value, day); };

  // columns
  if (!dm) {
    $('#cin', el).innerHTML = `<p class="empty">No run, messages or decisions for this module on day ${day}${m?.cadence && m.cadence !== 'daily' ? ` (it runs ${esc(m.cadence)})` : ''}.</p>`;
    $('#cthink', el).innerHTML = '<p class="empty">Did not think today.</p>'; $('#cout', el).innerHTML = '<p class="empty">Nothing.</p>';
  } else renderCols(el, ctx, dm, day);

  // strip
  const from = Math.max(R.first, day - 89), to = Math.min(R.last, Math.max(day, from + 89));
  const drawStrip = () => {
    const cells = []; let mx = 1, have = 0;
    for (let d = from; d <= to; d++) { const s = DI.get(d); const x = s?.m?.[id]; if (s) have++; cells.push([d, s, x]); if (x) mx = Math.max(mx, x[1] + x[2]); }
    $('#strip', el).innerHTML = cells.map(([d, s, x]) => {
      if (!s) return `<a href="${L.module(id, d)}" style="height:2px;background:var(--line-2)" title="day ${d} (loading)"${d === day ? ' class="cur"' : ''}></a>`;
      const h = x ? 6 + 28 * Math.sqrt((x[1] + x[2]) / mx) : 2;
      const col = x && x[0] >= 0 ? TIER_COL[TIERS[x[0]]] : x ? 'var(--muted)' : 'var(--line-2)';
      return `<a href="${L.module(id, d)}" class="${d === day ? 'cur' : ''}" style="height:${h.toFixed(0)}px;background:${col}" title="day ${d}: ${x ? `${x[1]} decision(s), ${x[2]} message(s), ${x[3]} deviation(s)` : 'idle'}"></a>`;
    }).join('');
    $('#stripnote', el).textContent = `days ${from}–${to}` + (have < cells.length ? ` · indexing ${have}/${cells.length}` : '');
  };
  drawStrip();
  const unsub = DI.subscribe(drawStrip);
  DI.scan(R);
  const onKey = e => { if (e.target.closest('input,textarea,select')) return; if (e.key === 'ArrowLeft' && day > R.first) location.hash = L.module(id, day - 1); if (e.key === 'ArrowRight' && day < R.last) location.hash = L.module(id, day + 1); };
  document.addEventListener('keydown', onKey);
  return () => { document.removeEventListener('keydown', onKey); unsub(); };
}

function renderCols(el, ctx, dm, day) {
  const L = ctx.link, run = dm.run;
  const inp = run?.inputs || {};
  const devById = new Map(dm.deviations.map(v => [v.id, v]));
  const msgIn = new Map(dm.messages_in.map(x => [x.id, x]));
  const devLi = idv => { const v = devById.get(idv); return v ? `<li><b>${esc(v.metric)}</b> ${v.z >= 0 ? '+' : ''}${fmt(v.z, 1)}σ <span class="faint">(${fmt(v.value)} vs ${fmt(v.baseline)})</span> L${v.level}${v.suppressed_by_budget ? ' <b style="color:var(--danger)">over budget</b>' : ''}<div class="meta">${esc(v.interpretation || '')}${v.disposition ? ` · ${esc(v.disposition.action)}: ${esc(v.disposition.reason)}` : ''}</div></li>` : `<li class="mono tiny">${esc(idv)}</li>`; };
  const msgLi = (x, dir) => `<li>${dir === 'in' ? ctx.nodeChip(x.from, day) : x.to.map(t => ctx.nodeChip(t, day)).join(' ')} <span class="chip">${esc(x.intent)}</span> <b>${esc(x.subject)}</b> <span class="faint tiny">d${x.day} hops ${x.hops}</span><details><summary>body</summary><div class="small">${md(x.body)}</div>${x.refs.length ? `<div class="tiny">refs: ${x.refs.map(refLink).join(' ')}</div>` : ''}</details></li>`;
  const sec = (t, body, cnt) => `<h4 class="tiny muted" style="margin:12px 0 2px;text-transform:uppercase;letter-spacing:.1em">${t}${cnt != null ? ` <span class="faint">${cnt}</span>` : ''}</h4>${body}`;
  const list = (arr, f, emp = 'none') => arr && arr.length ? `<ul class="list">${arr.map(f).join('')}</ul>` : `<p class="empty">${emp}</p>`;

  const metrics = inp.metrics ? Object.entries(inp.metrics) : [];
  $('#cin', el).innerHTML =
    sec('Routed deviations', list(inp.deviations, devLi), (inp.deviations || []).length) +
    (inp.queued_alerts?.length ? sec('Over-budget alerts to dispose', list(inp.queued_alerts, devLi), inp.queued_alerts.length) : '') +
    sec('Inbox', list(inp.messages, idm => msgIn.has(idm) ? msgLi(msgIn.get(idm), 'in') : `<li class="mono tiny">${esc(idm)}</li>`), (inp.messages || []).length) +
    (inp.due_switches?.length ? sec('Decision switches due', list(inp.due_switches, x => `<li><a href="${L.decision(x)}">${esc(x)}</a></li>`)) : '') +
    (inp.assumption_flags?.length ? sec('Assumption flags', list(inp.assumption_flags, x => `<li><a href="#/health?assumption=${encodeURIComponent(x)}">${esc(x)}</a></li>`)) : '') +
    (inp.okr_flags?.length ? sec('OKR flags', list(inp.okr_flags, x => `<li>${esc(x)}</li>`)) : '') +
    (inp.tasks?.length ? sec('Tasks', list(inp.tasks, x => `<li>${esc(typeof x === 'string' ? x : JSON.stringify(x))}</li>`)) : '') +
    (metrics.length ? sec('Metric snapshot', `<dl class="kv mono tiny">${metrics.map(([k, v]) => `<dt>${esc(k)}</dt><dd>${fmt(v)}</dd>`).join('')}</dl>`) : '') +
    (!run && dm.messages_in.length ? sec('Messages received', list(dm.messages_in, x => msgLi(x, 'in'))) : '');

  if (run) {
    const t = run.think;
    $('#cthink', el).innerHTML = `<div class="row"><span class="tier t-${t.tier}">${t.tier}</span><span class="tiny faint">${usd(run.cost_usd)} · ${fmt(run.tokens)} tokens · ${run.duration_ms} ms</span>${t.fallback ? `<span class="chip" style="color:#ffb35c;border-color:#ffb35c">fallback: ${esc(t.fallback)}</span>` : ''}</div>
      ${t.rationale ? `<div class="small" style="margin:8px 0">${md(t.rationale)}</div>` : ''}
      ${sec('Rules fired', t.rules_fired.length ? t.rules_fired.map(h => `<div class="rulehit"><b>${esc(h.class)}</b> <span class="faint">row ${h.row + 1}</span><div class="mono">when ${esc(h.when)}<br>then ${esc(h.then)}</div><div class="tiny muted">confidence ${fmt(h.confidence, 2)}${h.escalate ? ` · <b style="color:${TIER_COL[h.escalate] || 'inherit'}">escalate → ${esc(h.escalate)}</b>` : ''}</div>${Object.keys(h.bindings || {}).length ? `<div class="mono tiny faint">${Object.entries(h.bindings).map(([k, v]) => `${esc(k)}=${fmt(v)}`).join(' · ')}</div>` : ''}</div>`).join('') : '<p class="empty">No table row fired.</p>', t.rules_fired.length)}
      ${t.rules_failed.length ? sec('Rules failed / deferred', t.rules_failed.map(f => `<div class="rulehit fail small">${esc(typeof f === 'string' ? f : JSON.stringify(f))}</div>`).join('')) : ''}
      ${t.llm_request || t.llm_response ? sec('LLM call', `<div class="tiny muted">${esc(t.llm_model || '')}${t.prompt_ref ? ' · prompt <code>' + esc(t.prompt_ref) + '</code>' : ''}${t.llm?.cached ? ' · cached' : ''}</div>
        <details><summary>prompt</summary><pre class="code wrapc">${esc(t.llm_request || '')}</pre></details>
        <details open><summary>response</summary><pre class="code wrapc">${esc(pretty(t.llm_response))}</pre></details>`) : ''}`;
  } else $('#cthink', el).innerHTML = '<p class="empty">No think step today (cadence did not fire, or this node only received messages).</p>';

  const out = run?.outputs || {};
  const decs = dm.decisions, upd = dm.decision_updates;
  const decLi = x => `<li><a href="${L.decision(x.id)}">${esc(x.what)}</a><div class="meta"><span class="tier t-${x.tier}">${x.tier}</span> <span style="color:var(--muted)">${esc(x.status)}</span> · conf ${fmt(x.confidence, 2)} · €${fmt(x.stakes_eur)} · ${esc(RUNG_SHORT[x.rung_at_time] || x.rung_at_time)}${x.gate ? ` · gate ${x.gate.color}` : ''}${x.outcome ? ` · outcome ${x.outcome.success === true ? '✓' : x.outcome.success === false ? '✗' : '?'}` : ''}</div></li>`;
  $('#cout', el).innerHTML =
    sec('Decisions', list(decs, decLi), decs.length) +
    (upd.length ? sec('Decision updates', list(upd, decLi), upd.length) : '') +
    sec('Actions executed', list(dm.actions, a => `<li><b>${esc(a.kind)}</b> ${esc(a.effect_summary)} <a class="tiny" href="${L.decision(a.decision)}">${esc(a.decision)}</a></li>`), dm.actions.length) +
    sec('Messages sent', list(dm.messages_out, x => msgLi(x, 'out')), dm.messages_out.length) +
    (out.forecasts?.length ? sec('Forecasts', list(out.forecasts, f => `<li class="mono tiny">${esc(f)}</li>`)) : '') +
    (out.dismissed_alerts?.length ? sec('Alerts dismissed (logged act)', list(out.dismissed_alerts, x => `<li class="mono tiny">${esc(x)}</li>`)) : '') +
    (out.anomalies?.length ? sec('Anomalies noted', list(out.anomalies, x => `<li class="mono tiny">${esc(x)}</li>`)) : '');
}

function renderIndex(el, ctx) {
  const mods = ctx.meta.modules || [];
  const groups = groupBy(mods, x => x.tool ? 'OTA toolkit' : (x.org || 'other'));
  el.innerHTML = `<div class="wrap"><div class="ph"><div><div class="kicker">Module focus</div><h1>Pick a module</h1><p class="sub">Every AI module is an Observe → Think → Act loop with its own decision classes. Pick one to see what it saw, how it thought and what it did on any day.</p></div></div>
    ${groups.map(([g, list]) => `<section class="sect"><h2 style="font-size:17px">${esc(g)}</h2><div class="howto">${list.map(m => `<a class="panel" href="${ctx.link.module(m.id, ctx.day)}"><b>${esc(m.name || m.id)}</b><div class="tiny muted">${(m.phases || []).map(p => `<span style="color:${PHASE_COL[p]}">${p}</span>`).join(' · ')} · ${esc(m.cadence)} · ${(m.classes || []).length} classes · accountable ${esc(ctx.name(m.accountable))}</div>${m.evolves_from ? `<div class="tiny faint">evolves from ${esc(m.evolves_from)}</div>` : ''}</a>`).join('')}</div></section>`).join('')}
    <section class="sect"><h2 style="font-size:17px">People</h2><p class="intro">Humans have their own page: <a href="#/people">People</a>.</p></section></div>`;
}

export function tableHTML(c) {
  return `<h5 style="margin:14px 0 4px;color:var(--gold)">${esc(c.id)}</h5><div class="tiny muted">confirm_if <code>${esc(c.confirm_if || '–')}</code> · stop_if <code>${esc(c.stop_if || '–')}</code> · check after ${c.check_after_days ?? '–'} d</div>
    <div class="tscroll"><table class="t mono tiny"><thead><tr><th>#</th><th>when</th><th>then</th><th>conf</th><th>escalate</th><th>note</th></tr></thead><tbody>
    ${(c.rows || []).map((r, i) => `<tr><td>${i + 1}</td><td>${esc(r.when)}</td><td>${esc(r.then)}</td><td>${r.confidence ?? ''}</td><td>${r.escalate ? `<span class="tier t-${esc(r.escalate)}">${esc(r.escalate)}</span>` : ''}</td><td class="faint">${esc(r.note || '')}</td></tr>`).join('')}</tbody></table></div>`;
}
export function rungBar(r) {
  const i = RUNGS.indexOf(r);
  return `<span title="${esc(RUNG_SHORT[r] || r)}" style="display:inline-flex;gap:2px;vertical-align:middle">${RUNGS.map((_, k) => `<i style="width:9px;height:9px;border-radius:2px;background:${k <= i ? 'var(--gold)' : 'var(--line-2)'}"></i>`).join('')}</span> <span class="tiny">${esc(RUNG_SHORT[r] || r)}</span>`;
}
export function refLink(x) {
  if (/^D-\d/.test(x)) return `<a class="mono" href="#/decision/${encodeURIComponent(x)}">${esc(x)}</a>`;
  return `<span class="mono faint">${esc(x)}</span>`;
}
function pretty(s) { if (!s) return ''; try { return JSON.stringify(JSON.parse(s), null, 2); } catch { return s; } }
export function groupBy(arr, f) { const m = new Map(); for (const x of arr) { const k = f(x); if (!m.has(k)) m.set(k, []); m.get(k).push(x); } return [...m]; }
