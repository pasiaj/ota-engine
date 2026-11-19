// Health: is the loop itself healthy? Sentinel timeline over all pathogens / failure modes / signal
// classes, the quarterly success grid, autonomy over time, aggregate calibration, assumptions and
// graveyard, scenario tree, coherence gaps, OKRs, and the human footprint (real-human share first).
// Individual human calibration is private and never shown (§13.5).
import { api } from '../api.js';
import { esc, fmt, pct, humanize, spark, minutes, $ } from '../util.js';
import { PATHOGENS, FAILURE_MODES, SIGNAL_CLASSES, SENTINEL_RULES, RUNGS, RUNG_SHORT, MODALITIES, PHASES, kindVocab, vocab } from '../catalog.js';
import { headline } from '../kpis.js';

export async function render(el, ctx, rt) {
  const [S, H] = await Promise.all([api.state(), headline()]);
  if (!rt.isCurrent()) return;
  const R = ctx.days, cur = rt.day || ctx.day || R.last;
  const i = H.days.indexOf(cur);
  const rhs = H.ota.real_human_share;
  const lastRhs = [...rhs].reverse().find(v => v != null);
  const hm = H.ota.human_minutes;
  const sumHm = hm.reduce((a, b) => a + (b || 0), 0);
  const realActs = (S.queue || []).length;
  el.innerHTML = `<div class="wrap">
    <div class="ph"><div><div class="kicker">Health · state as of day ${S.day} (${esc(S.date)})</div><h1>Is the loop healthy?</h1><p class="sub">The toolkit watches the company; Sentinel watches the toolkit; the independent audit watches Sentinel. Cursor at day ${cur}.</p></div>
      <div class="tools"><a class="btn" href="#/health/day/${Math.max(R.first, cur - 1)}">‹</a><a class="btn" href="#/health/day/${Math.min(R.last, cur + 1)}">›</a></div></div>

    <section class="panel" data-tour="footprint"><header><h3>Human footprint</h3>${ctx.help('honest', 'Why real-human share matters')}</header>
      <div class="bigs">
        <div class="big"><span>Real-human share</span><b style="color:${lastRhs > 0 ? 'var(--ok)' : 'var(--danger)'};font-size:34px">${pct(lastRhs, 1)}</b><small>of human acts (30 days) by a real person; the rest are LLM stand-ins</small></div>
        <div class="big"><span>Human minutes, all days</span><b>${minutes(sumHm)}</b><small>${fmt(sumHm / Math.max(1, H.days.length))} min/day across ${ctx.meta.people.length} people</small></div>
        <div class="big"><span>Open human queue</span><b>${realActs}</b><small>items waiting for a person</small></div>
        <div class="big"><span>Ledger completeness</span><b>${pct(lastOf(H.ota.ledger_completeness))}</b><small>7 fields filled, 30 days</small></div>
        <div class="big"><span>External signal share</span><b>${pct(lastOf(H.ota.external_signal_share))}</b><small>echo-chamber guard</small></div>
        <div class="big"><span>AI Brier, 30 days</span><b>${fmt(lastOf(H.ota.brier_30d), 3)}</b><small>lower is better · 0.25 = coin flip</small></div>
      </div>
      <div class="sparks"><div class="s"><div class="l"><span>Real-human share</span><b>${pct(rhs[i], 1)}</b></div>${spark(rhs, { cursor: i, color: 'var(--ok)', min: 0 })}</div>
        <div class="s"><div class="l"><span>Human minutes / day</span><b>${minutes(hm[i])}</b></div>${spark(hm, { cursor: i, color: '#f1d27a' })}</div>
        <div class="s"><div class="l"><span>Brier 30d (AI)</span><b>${fmt(H.ota.brier_30d[i], 3)}</b></div>${spark(H.ota.brier_30d, { cursor: i, color: '#a98bff' })}</div>
        <div class="s"><div class="l"><span>Alerts over budget</span><b>${fmt(H.ota.alerts_suppressed[i])}</b></div>${spark(H.ota.alerts_suppressed, { cursor: i, color: 'var(--danger)' })}</div>
        <div class="s"><div class="l"><span>Decision latency (d)</span><b>${fmt(H.ota.decision_latency_days[i])}</b></div>${spark(H.ota.decision_latency_days, { cursor: i, color: '#8cc8ff' })}</div>
        <div class="s"><div class="l"><span>Hops to decider</span><b>${fmt(H.ota.hops_to_decider[i])}</b></div>${spark(H.ota.hops_to_decider, { cursor: i, color: '#ff9a3d' })}</div></div>
    </section>

    <section class="panel" style="margin-top:14px" data-tour="sentinel"><header><h3>Sentinel findings over time</h3><span class="r tiny faint">${S.sentinel.length} findings (newest 300)</span>${ctx.help('sentinel')}</header><div id="sentl"></div></section>

    <div class="grid g2" style="margin-top:14px">
      <section class="panel" data-tour="grid"><header><h3>Success grid</h3><span class="r" id="gq"></span>${ctx.help('grid')}</header><div id="grid"></div></section>
      <section class="panel"><header><h3>Calibration (aggregate &amp; AI only)</h3>${ctx.help('tool-calibration')}</header><div id="calib"></div></section>
    </div>
    <section class="panel" style="margin-top:14px"><header><h3>Autonomy over time</h3><span class="r tiny faint">class × day, colour = rung</span>${ctx.help('autonomy')}</header><div id="auto"></div></section>
    <div class="grid g2" style="margin-top:14px">
      <section class="panel" id="assump"><header><h3>Assumptions</h3><span class="r tiny">${S.graveyard.length ? `<b style="color:var(--danger)">${S.graveyard.length} in the graveyard</b>` : 'graveyard empty'}</span>${ctx.help('tool-assumptions')}</header><div id="asl"></div></section>
      <section class="panel"><header><h3>Living scenario tree</h3>${ctx.help('tool-scenarios')}</header><div id="scen"></div></section>
      <section class="panel"><header><h3>Living OKRs</h3>${ctx.help('tool-okrs')}</header><div id="okrs"></div></section>
      <section class="panel"><header><h3>Coherence &amp; learning findings</h3><span class="r tiny">gap ${S.coherence_gap == null ? '–' : fmt(S.coherence_gap, 2)}</span>${ctx.help('tool-coherence')}</header><div id="finds"></div></section>
    </div>
  </div>`;

  // --- Sentinel timeline: every kind is a row, even if it never fired.
  const rows = [
    ...Object.keys(PATHOGENS).map(n => ['pathogen', n]), ...Object.keys(FAILURE_MODES).map(n => ['failure_mode', n]),
    ...Object.keys(SIGNAL_CLASSES).map(n => ['signal_class', n]), ...Object.keys(SENTINEL_RULES).map(n => ['rule', n])
  ];
  for (const s of S.sentinel) if (!rows.some(([c, n]) => c === s.kind.category && n === s.kind.name)) rows.push([s.kind.category, s.kind.name]);
  const cat = { pathogen: '#b5d33a', failure_mode: '#7cf2ff', signal_class: '#ffc94a', rule: '#ff9a3d' };
  const W = 1000, rh = 15, lw = 190, span = Math.max(1, R.last - R.first);
  const X = d => lw + (d - R.first) / span * (W - lw - 10);
  let svg = `<svg viewBox="0 0 ${W} ${rows.length * rh + 24}" width="100%" style="min-width:560px">`;
  let lastCat = '';
  rows.forEach(([c, n], k) => {
    const y = k * rh + 12;
    if (c !== lastCat) { svg += `<line x1="0" x2="${W}" y1="${y - 8}" y2="${y - 8}" stroke="var(--line-2)"/>`; lastCat = c; }
    const hits = S.sentinel.filter(s => s.kind.category === c && s.kind.name === n);
    svg += `<a href="${vocab(kindVocab(n))}" target="_blank" rel="noopener"><text x="4" y="${y + 3}" font-size="10.5" fill="${hits.length ? 'var(--text)' : 'var(--faint)'}">${esc(humanize(n))}<title>${esc(humanize(c))}: ${esc(PATHOGENS[n] || FAILURE_MODES[n] || SIGNAL_CLASSES[n] || SENTINEL_RULES[n] || '')}</title></text></a>`;
    svg += `<line x1="${lw}" x2="${W - 10}" y1="${y}" y2="${y}" stroke="var(--line-2)" stroke-dasharray="1 4"/>`;
    for (const h of hits) svg += `<circle cx="${X(h.day).toFixed(1)}" cy="${y}" r="${h.severity === 'high' ? 5 : h.severity === 'medium' ? 4 : 3}" fill="${cat[c] || '#ccc'}" fill-opacity=".8"><title>day ${h.day}: ${esc(h.signature)}</title></circle>`;
  });
  svg += `<line x1="${X(cur)}" x2="${X(cur)}" y1="0" y2="${rows.length * rh + 10}" stroke="var(--gold-2)" stroke-opacity=".6"/>`;
  const tools = ctx.meta.toolkit || [];
  const sen = tools.find(t => t.id === 'sentinel');
  if (sen) svg += `<line x1="${X(sen.enabled_from)}" x2="${X(sen.enabled_from)}" y1="0" y2="${rows.length * rh + 10}" stroke="var(--danger)" stroke-dasharray="3 3" stroke-opacity=".6"/><text x="${X(sen.enabled_from) + 4}" y="${rows.length * rh + 20}" font-size="10" fill="var(--danger)">Sentinel on (day ${sen.enabled_from})</text>`;
  svg += '</svg>';
  const recent = S.sentinel.filter(s => s.day <= cur).slice(0, 8);
  $('#sentl', el).innerHTML = `<div class="tscroll">${svg}</div><div class="legend" style="margin:6px 0"><span><i class="dot" style="background:${cat.pathogen}"></i>pathogen (6)</span><span><i class="dot" style="background:${cat.failure_mode}"></i>AI failure mode (13)</span><span><i class="dot" style="background:${cat.signal_class}"></i>signal class (4)</span><span><i class="dot" style="background:${cat.rule}"></i>rule</span><span class="faint">names link to the book atlas</span></div>
    <details><summary>Latest findings up to day ${cur}</summary><ul class="list">${recent.map(s => `<li><b>${esc(humanize(s.kind.name))}</b> <span class="chip">${esc(s.severity)}</span> <span class="chip">${esc(s.status)}</span> <a href="#/birdseye/day/${s.day}">day ${s.day}</a><div class="meta">${esc(s.signature)}${s.evidence.length ? ' · evidence ' + s.evidence.slice(0, 6).map(x => /^D-/.test(x) ? `<a href="#/decision/${x}">${esc(x)}</a>` : esc(x)).join(', ') : ''}</div></li>`).join('') || '<li class="empty">none</li>'}</ul></details>`;

  // --- Success grid
  const grids = S.grid || [];
  let gsel = grids.length - 1;
  const drawGrid = () => {
    const g = grids[gsel];
    $('#gq', el).innerHTML = grids.map((x, k) => `<button class="btn${k === gsel ? ' on' : ''}" data-k="${k}" style="padding:3px 8px">${esc(x.quarter)}</button>`).join('');
    el.querySelectorAll('#gq button').forEach(b => b.onclick = () => { gsel = +b.dataset.k; drawGrid(); });
    if (!g) { $('#grid', el).innerHTML = '<p class="empty">No quarterly self-assessment yet.</p>'; return; }
    const col = v => `hsl(${Math.round(v * 120)} 55% ${30 + v * 22}%)`;
    let h = `<div class="gridsc"><div></div>${MODALITIES.map(m => `<div class="h" title="${m}"><a href="${vocab(m)}" target="_blank" rel="noopener" style="color:inherit">${m}</a></div>`).join('')}`;
    g.cells.forEach((row, pi) => { h += `<div class="h" style="text-align:right">${PHASES[pi]}</div>` + row.map((v, mi) => `<div class="c${pi === 1 && mi === 0 ? ' anchor' : ''}" style="background:${col(v)}" title="${PHASES[pi]} × ${MODALITIES[mi]}: health ${fmt(v, 2)} · residual risk ${fmt(g.residual_risk?.[pi]?.[mi], 1)}">${fmt(v, 2)}</div>`).join(''); });
    h += '</div>';
    h += `<dl class="kv small" style="margin-top:10px"><dt>universal anchor</dt><dd>${fmt(g.universal_anchor, 2)} <span class="faint">(think × direction, outlined)</span></dd><dt>culture trap</dt><dd>${fmt(g.culture_trap, 2)} <span class="faint">(mean of culture column)</span></dd>
      <dt>nearest OTA 200 profile</dt><dd><b>${esc(humanize(g.nearest_profile))}</b> <span class="faint">${Object.entries(g.profile_distance || {}).map(([k, v]) => `${humanize(k)} ${fmt(v, 1)}`).join(' · ')}</span></dd><dt>archetype</dt><dd>${esc(g.archetype)}</dd></dl>
      <details><summary>Residual risk (OTA 200 failure weight × weakness)</summary><div class="gridsc" style="margin-top:6px"><div></div>${MODALITIES.map(m => `<div class="h">${m}</div>`).join('')}${g.residual_risk.map((row, pi) => `<div class="h" style="text-align:right">${PHASES[pi]}</div>` + row.map(v => `<div class="c" style="background:rgba(255,93,93,${Math.min(.9, v / 30 + .05)});color:var(--text)">${fmt(v, 1)}</div>`).join('')).join('')}</div></details>
      ${g.notes?.length ? `<ul class="list tiny">${g.notes.map(n => `<li>${esc(n)}</li>`).join('')}</ul>` : ''}`;
    $('#grid', el).innerHTML = h;
  };
  drawGrid();

  // --- Calibration (reliability diagram of aggregate AI and aggregate human)
  const C = S.calibration || {};
  const rel = (p, color) => (p?.bins || []).map(b => `<circle cx="${20 + b.mean_p * 200}" cy="${210 - b.freq * 200}" r="${Math.min(9, 2 + Math.sqrt(b.n))}" fill="${color}" fill-opacity=".6"><title>p ${fmt(b.p_lo, 1)}–${fmt(b.p_hi, 1)}: n=${b.n}, said ${fmt(b.mean_p, 2)}, happened ${fmt(b.freq, 2)}</title></circle>`).join('');
  const byMod = Object.entries(C.by_module || {}).filter(([, p]) => p.n).sort((a, b) => b[1].n - a[1].n);
  $('#calib', el).innerHTML = `<div class="row" style="align-items:flex-start"><svg viewBox="0 0 230 230" width="210" style="flex:0 0 auto;max-width:100%"><rect x="20" y="10" width="200" height="200" fill="none" stroke="var(--line)"/><line x1="20" y1="210" x2="220" y2="10" stroke="var(--faint)" stroke-dasharray="3 3"/>${rel(C.overall_human, '#f1d27a')}${rel(C.overall_ai, '#8cc8ff')}<text x="120" y="226" font-size="9" fill="var(--muted)" text-anchor="middle">stated probability</text><text x="10" y="110" font-size="9" fill="var(--muted)" transform="rotate(-90 10 110)" text-anchor="middle">observed frequency</text></svg>
    <dl class="kv small" style="flex:1 1 180px"><dt><span class="dot" style="background:#8cc8ff"></span> AI overall</dt><dd>n ${C.overall_ai?.n ?? 0} · Brier ${fmt(C.overall_ai?.brier, 3)} · base rate ${pct(C.overall_ai?.base_rate)}</dd><dt><span class="dot" style="background:#f1d27a"></span> humans (aggregate)</dt><dd>n ${C.overall_human?.n ?? 0} · Brier ${fmt(C.overall_human?.brier, 3)}</dd><dt>open forecasts</dt><dd>${C.open_forecasts ?? 0}</dd></dl></div>
    <p class="tiny faint">Per-person calibration is private to each person (§13.5) and is not shown here.</p>
    ${byMod.length ? `<div class="tscroll"><table class="t small"><thead><tr><th>module</th><th class="num">n</th><th class="num">Brier</th><th class="num">mean p</th><th class="num">base rate</th></tr></thead><tbody>${byMod.map(([m, p]) => `<tr><td><a href="${ctx.link.module(m)}">${esc(ctx.name(m))}</a></td><td class="num">${p.n}</td><td class="num" style="color:${p.brier > .25 ? 'var(--danger)' : 'inherit'}">${fmt(p.brier, 3)}</td><td class="num">${fmt(p.mean_p, 2)}</td><td class="num">${pct(p.base_rate)}</td></tr>`).join('')}</tbody></table></div>` : ''}`;

  // --- Autonomy over time: reconstruct rung per class from defaults + approved changes.
  const classes = ctx.meta.classes || [];
  const hist = (S.autonomy?.history || []).filter(a => a.status === 'approved').sort((a, b) => a.day - b.day);
  const changed = new Set(hist.map(a => a.class));
  const rungCol = r => ['#3a3f4a', '#6a7a99', '#c9a03f', '#7ac68a', '#3fd0c9'][RUNGS.indexOf(r)] || '#333';
  const showCls = [...classes].sort((a, b) => (changed.has(b.id) - changed.has(a.id)) || a.id.localeCompare(b.id));
  const aw = 1000, ah = 12, al = 230;
  const AX = d => al + (d - R.first) / span * (aw - al - 6);
  const autOn = tools.find(t => t.id === 'autonomy')?.enabled_from;
  let as = `<svg viewBox="0 0 ${aw} ${showCls.length * ah + 20}" width="100%" style="min-width:560px">`;
  showCls.forEach((c, k) => {
    const y = k * ah + 4; let r = c.default_rung, from = R.first;
    as += `<a href="#/ledger?class=${encodeURIComponent(c.id)}"><text x="${al - 6}" y="${y + 9}" font-size="9.5" text-anchor="end" fill="${changed.has(c.id) ? 'var(--text)' : 'var(--faint)'}">${esc(c.id)}</text></a>`;
    for (const a of hist.filter(a => a.class === c.id)) { as += `<rect x="${AX(from)}" y="${y}" width="${Math.max(0, AX(a.day) - AX(from))}" height="${ah - 2}" fill="${rungCol(r)}"><title>${esc(c.id)}: ${RUNG_SHORT[r]} (days ${from}–${a.day})</title></rect>`; r = a.to; from = a.day; }
    as += `<rect x="${AX(from)}" y="${y}" width="${Math.max(0, AX(R.last) - AX(from))}" height="${ah - 2}" fill="${rungCol(r)}"><title>${esc(c.id)}: ${RUNG_SHORT[r]} (from day ${from})</title></rect>`;
  });
  if (autOn) as += `<line x1="${AX(autOn)}" x2="${AX(autOn)}" y1="0" y2="${showCls.length * ah + 4}" stroke="var(--gold-2)" stroke-dasharray="3 3"/><text x="${AX(autOn) + 3}" y="${showCls.length * ah + 15}" font-size="10" fill="var(--gold-2)">Autonomy Ladder on (day ${autOn})</text>`;
  as += `<line x1="${AX(cur)}" x2="${AX(cur)}" y1="0" y2="${showCls.length * ah + 4}" stroke="#fff" stroke-opacity=".5"/></svg>`;
  const recentAu = (S.autonomy?.history || []).slice(-8).reverse();
  $('#auto', el).innerHTML = `<div class="legend">${RUNGS.map(r => `<span><i class="dot" style="background:${rungCol(r)};border-radius:2px"></i>${RUNG_SHORT[r]}</span>`).join('')}</div><div class="tscroll" style="max-height:520px;overflow-y:auto">${as}</div>
    <details><summary>Recent autonomy changes (${(S.autonomy?.history || []).length} total)</summary><ul class="list">${recentAu.map(a => `<li><b>${esc(a.class)}</b> ${esc(RUNG_SHORT[a.from])} → ${esc(RUNG_SHORT[a.to])} · ${esc(a.status)} · ${esc(humanize(a.trigger))} · day ${a.day}<div class="meta">Brier ${fmt(a.evidence?.brier, 3)} (n ${a.evidence?.n}) · agreement ${pct(a.evidence?.agreement_rate)} · threshold ${fmt(a.evidence?.threshold, 2)}${a.approved_by ? ' · approved by ' + esc(ctx.name(a.approved_by)) + ` <span class="${a.source === 'real_human' ? 'src-real' : 'src-standin'}">(${esc(a.source)})</span>` : ''}</div></li>`).join('') || '<li class="empty">none yet</li>'}</ul></details>`;

  // --- Assumptions
  const focus = rt.query.get('assumption');
  const grave = new Set(S.graveyard);
  const as2 = [...S.assumptions].sort((a, b) => (grave.has(b.id) - grave.has(a.id)) || (b.load_bearing - a.load_bearing) || a.confidence - b.confidence);
  const stCol = { active: 'var(--muted)', confirmed: 'var(--ok)', refuted: 'var(--danger)', stale: '#ffb35c', archived: 'var(--faint)' };
  $('#asl', el).innerHTML = `<div style="max-height:460px;overflow:auto"><ul class="list">${as2.map(a => `<li id="as-${esc(a.id)}" style="${a.id === focus ? 'background:rgba(217,180,92,.12);' : ''}"><div class="row"><b class="mono tiny">${esc(a.id)}</b><span class="st" style="color:${stCol[a.status]}">${esc(a.status)}</span>${grave.has(a.id) ? '<span class="chip" style="color:var(--danger);border-color:var(--danger)">graveyard</span>' : ''}${a.load_bearing ? '<span class="chip">load-bearing</span>' : ''}<span class="sp"></span><span style="width:90px">${spark(a.history.map(h => h.confidence), { w: 90, h: 16, min: 0, max: 1, fill: false, color: stCol[a.status] })}</span><b>${pct(a.confidence)}</b></div><div class="small">${esc(a.text)}</div><div class="meta">owner ${esc(ctx.name(a.owner))} · last confirmed day ${a.last_confirmed}${a.stale_since ? ' · stale since day ' + a.stale_since : ''}</div></li>`).join('') || '<li class="empty">No assumptions.</li>'}</ul></div>`;
  if (focus) setTimeout(() => document.getElementById('as-' + focus)?.scrollIntoView({ block: 'center' }), 50);

  // --- Scenario tree (DAG drawn as a tree on the first parent)
  const sc = S.scenarios || [];
  const kids = new Map(); const roots = [];
  for (const s of sc) { const p = s.parent?.[0]; if (p && sc.some(x => x.id === p)) { if (!kids.has(p)) kids.set(p, []); kids.get(p).push(s); } else roots.push(s); }
  const scCol = { approved: 'var(--ok)', pending: '#ffc94a', logged: 'var(--muted)', rejected: 'var(--danger)' };
  const node = (s, depth) => `<li style="padding-left:${depth * 16}px"><div class="row"><span class="st" style="color:${scCol[s.status] || 'inherit'}">${esc(s.status)}</span><span class="chip">${esc(s.tier)}</span><span style="flex:1;min-width:120px">${esc(s.title)}</span><span style="width:70px" class="bar"><i style="width:${(s.p * 100).toFixed(0)}%"></i></span><b class="tiny">${pct(s.p)}</b></div><div class="meta">${esc(s.proposed_by || '')} · day ${s.created}${s.approved_by ? ' · approved by ' + esc(ctx.name(s.approved_by)) : ''}${s.leading_indicators?.length ? ' · ' + s.leading_indicators.map(x => `<code>${esc(x.expr)}</code>`).join(' ') : ''}</div></li>` + (kids.get(s.id) || []).map(k => node(k, depth + 1)).join('');
  $('#scen', el).innerHTML = sc.length ? `<div style="max-height:460px;overflow:auto"><ul class="list">${roots.map(s => node(s, 0)).join('')}</ul></div>` : '<p class="empty">No scenarios.</p>';

  // --- OKRs
  $('#okrs', el).innerHTML = (S.okrs || []).length ? `<ul class="list">${S.okrs.map(o => `<li><b>${esc(o.id)}</b> ${esc(o.objective)} <span class="faint tiny">setter ${esc(ctx.name(o.setter))}</span>${o.flagged ? `<div class="meta" style="color:#ffb35c">⚑ flagged day ${o.flagged_day}: ${esc(o.flagged)}</div>` : ''}
    ${o.key_results.map(k => { const prog = k.current == null ? null : k.direction === 'down' ? Math.min(1.2, k.target / k.current) : Math.min(1.2, k.current / k.target); return `<div class="hbar"><span class="l mono tiny" title="${esc(k.metric)}">${esc(k.metric)}</span><span class="b"><i style="width:${prog == null ? 0 : Math.min(100, prog * 100)}%;background:${prog >= 1 ? 'var(--ok)' : prog > .85 ? '#ffc94a' : 'var(--danger)'}"></i></span><span class="v tiny">${fmt(k.current)} / ${fmt(k.target)}</span></div>`; }).join('')}</li>`).join('')}</ul>` : '<p class="empty">No OKRs.</p>';

  // --- Findings (coherence, double loop, audit, meta loop)
  const F = S.findings || [];
  const tcol = { coherence: '#6aa8ff', double_loop: '#5fd38d', audit: '#7fb2c9', meta_loop: '#e276d6' };
  $('#finds', el).innerHTML = F.length ? `<div style="max-height:460px;overflow:auto"><ul class="list">${[...F].reverse().slice(0, 60).map(f => `<li><span class="chip" style="color:${tcol[f.tool] || 'inherit'}">${esc(humanize(f.tool))}</span> <b>${esc(humanize(f.kind))}</b> <a class="tiny" href="#/birdseye/day/${f.day}">day ${f.day}</a><div class="small">${esc(f.text)}</div></li>`).join('')}</ul></div>` : '<p class="empty">No findings yet.</p>';
}

function lastOf(a) { for (let k = a.length - 1; k >= 0; k--) if (a[k] != null) return a[k]; return null; }
