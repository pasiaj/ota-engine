// Small helpers shared by every view: escaping, formatting, Markdown, SVG charts, storage.

export const $ = (s, r = document) => r.querySelector(s);
export const $$ = (s, r = document) => Array.from(r.querySelectorAll(s));

export function esc(s) {
  return String(s ?? '').replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
}

export function fmt(v, d) {
  if (v == null || Number.isNaN(v)) return '–';
  if (typeof v !== 'number') return esc(v);
  const a = Math.abs(v);
  if (d != null) return v.toLocaleString('en-US', { maximumFractionDigits: d, minimumFractionDigits: d });
  if (a >= 1e6) return (v / 1e6).toFixed(a >= 1e7 ? 1 : 2) + 'M';
  if (a >= 1e4) return (v / 1e3).toFixed(a >= 1e5 ? 0 : 1) + 'k';
  if (a >= 100) return Math.round(v).toLocaleString('en-US');
  if (a >= 1) return v.toFixed(2).replace(/\.?0+$/, '');
  if (a === 0) return '0';
  return v.toPrecision(2);
}
export const pct = (v, d = 0) => v == null ? '–' : (v * 100).toFixed(d) + '%';
export const usd = v => v == null ? '–' : '$' + (v < 0.01 && v > 0 ? v.toFixed(4) : v.toFixed(2));
export const eur = v => v == null ? '–' : '€' + fmt(v);
export const minutes = m => m == null ? '–' : m >= 90 ? (m / 60).toFixed(1) + ' h' : Math.round(m) + ' min';
export const clock = ts => ts == null ? '' : String(Math.floor(ts / 60)).padStart(2, '0') + ':' + String(ts % 60).padStart(2, '0');
export const humanize = s => String(s ?? '').replace(/_/g, ' ');

// Tiny, safe Markdown → HTML (headings, lists, tables, code, emphasis, links). Input is escaped first.
export function md(src) {
  if (!src) return '';
  const lines = String(src).replace(/\r/g, '').split('\n');
  const out = []; let i = 0;
  const inline = t => esc(t)
    .replace(/`([^`]+)`/g, '<code>$1</code>')
    .replace(/\*\*([^*]+)\*\*/g, '<b>$1</b>')
    .replace(/(^|[^*])\*([^*\s][^*]*)\*/g, '$1<i>$2</i>')
    .replace(/\[([^\]]+)\]\((https?:[^)\s]+|#[^)\s]*)\)/g, '<a href="$2">$1</a>');
  while (i < lines.length) {
    const l = lines[i];
    if (/^```/.test(l)) { const b = []; i++; while (i < lines.length && !/^```/.test(lines[i])) b.push(lines[i++]); i++; out.push('<pre class="code">' + esc(b.join('\n')) + '</pre>'); continue; }
    const hm = l.match(/^(#{1,4})\s+(.*)/);
    if (hm) { const n = Math.min(4, hm[1].length + 2); out.push(`<h${n}>${inline(hm[2])}</h${n}>`); i++; continue; }
    if (/^\s*\|/.test(l) && i + 1 < lines.length && /^\s*\|[\s:|-]+\|?\s*$/.test(lines[i + 1])) {
      const row = r => r.trim().replace(/^\||\|$/g, '').split('|').map(c => c.trim());
      const head = row(l); i += 2; const rows = [];
      while (i < lines.length && /^\s*\|/.test(lines[i])) rows.push(row(lines[i++]));
      out.push('<div class="tscroll"><table class="t"><thead><tr>' + head.map(c => `<th>${inline(c)}</th>`).join('') + '</tr></thead><tbody>' +
        rows.map(r => '<tr>' + r.map(c => `<td>${inline(c)}</td>`).join('') + '</tr>').join('') + '</tbody></table></div>');
      continue;
    }
    if (/^\s*([-*]|\d+\.)\s+/.test(l)) {
      const ordered = /^\s*\d+\./.test(l); const items = [];
      while (i < lines.length && /^\s*([-*]|\d+\.)\s+/.test(lines[i])) {
        let it = lines[i++].replace(/^\s*([-*]|\d+\.)\s+/, '');
        while (i < lines.length && /^\s{2,}\S/.test(lines[i]) && !/^\s*([-*]|\d+\.)\s+/.test(lines[i])) it += ' ' + lines[i++].trim();
        items.push(`<li>${inline(it)}</li>`);
      }
      out.push(ordered ? `<ol>${items.join('')}</ol>` : `<ul>${items.join('')}</ul>`); continue;
    }
    if (/^>\s?/.test(l)) { const b = []; while (i < lines.length && /^>\s?/.test(lines[i])) b.push(lines[i++].replace(/^>\s?/, '')); out.push('<blockquote>' + inline(b.join(' ')) + '</blockquote>'); continue; }
    if (!l.trim()) { i++; continue; }
    const p = [l]; i++;
    while (i < lines.length && lines[i].trim() && !/^(#|```|\s*\||\s*([-*]|\d+\.)\s|>)/.test(lines[i])) p.push(lines[i++]);
    out.push('<p>' + inline(p.join(' ')) + '</p>');
  }
  return out.join('\n');
}

// Sparkline: values (nulls allowed) → inline SVG path. marks: [{i, color, title}]; cursor index.
export function spark(vals, { w = 160, h = 34, color = 'var(--gold)', cursor = null, fill = true, marks = [], min = null, max = null } = {}) {
  const v = vals.map(x => (x == null || !isFinite(x)) ? null : +x);
  const nums = v.filter(x => x != null);
  if (!nums.length) return `<svg class="spark" viewBox="0 0 ${w} ${h}" width="100%" height="${h}" preserveAspectRatio="none"></svg>`;
  const lo = min ?? Math.min(...nums), hi = max ?? Math.max(...nums), span = hi - lo || 1;
  const X = i => v.length < 2 ? w / 2 : i / (v.length - 1) * w;
  const Y = x => h - 2 - (x - lo) / span * (h - 4);
  let d = '', pen = false;
  v.forEach((x, i) => { if (x == null) { pen = false; return; } d += (pen ? 'L' : 'M') + X(i).toFixed(1) + ' ' + Y(x).toFixed(1); pen = true; });
  const area = fill && nums.length > 1 ? `<path d="${d}L${w} ${h}L0 ${h}Z" fill="${color}" opacity=".10" stroke="none"/>` : '';
  const mk = marks.map(m => `<line x1="${X(m.i).toFixed(1)}" x2="${X(m.i).toFixed(1)}" y1="0" y2="${h}" stroke="${m.color || 'var(--faint)'}" stroke-width="1" opacity=".7"><title>${esc(m.title || '')}</title></line>`).join('');
  const cur = cursor != null ? `<line x1="${X(cursor).toFixed(1)}" x2="${X(cursor).toFixed(1)}" y1="0" y2="${h}" stroke="var(--gold-2)" stroke-width="1.5"/>` +
    (v[cursor] != null ? `<circle cx="${X(cursor).toFixed(1)}" cy="${Y(v[cursor]).toFixed(1)}" r="2.6" fill="var(--gold-2)"/>` : '') : '';
  return `<svg class="spark" viewBox="0 0 ${w} ${h}" width="100%" height="${h}" preserveAspectRatio="none">${area}${mk}<path d="${d}" fill="none" stroke="${color}" stroke-width="1.5" vector-effect="non-scaling-stroke"/>${cur}</svg>`;
}

// Stacked bars per index: series = [{vals, color, name}]
export function stackBars(series, { w = 600, h = 60, cursor = null } = {}) {
  const n = series[0]?.vals.length || 0; if (!n) return '';
  const tot = Array.from({ length: n }, (_, i) => series.reduce((s, x) => s + (+x.vals[i] || 0), 0));
  const mx = Math.max(1, ...tot), bw = w / n;
  let r = '';
  for (let i = 0; i < n; i++) {
    let y = h;
    for (const s of series) { const v = +s.vals[i] || 0; if (!v) continue; const bh = v / mx * (h - 2); y -= bh; r += `<rect x="${(i * bw).toFixed(2)}" y="${y.toFixed(2)}" width="${Math.max(bw - .3, .6).toFixed(2)}" height="${bh.toFixed(2)}" fill="${s.color}"/>`; }
  }
  const cur = cursor != null ? `<rect x="${(cursor * bw).toFixed(2)}" y="0" width="${Math.max(bw, 1.5).toFixed(2)}" height="${h}" fill="var(--gold-2)" opacity=".35"/>` : '';
  return `<svg class="spark" viewBox="0 0 ${w} ${h}" width="100%" height="${h}" preserveAspectRatio="none">${r}${cur}</svg>`;
}

// localStorage that never throws (private windows, blocked storage).
export const store = {
  get(k) { try { return localStorage.getItem(k); } catch { return null; } },
  set(k, v) { try { v == null ? localStorage.removeItem(k) : localStorage.setItem(k, v); } catch { /* ignore */ } }
};

export const debounce = (f, ms) => { let t; return (...a) => { clearTimeout(t); t = setTimeout(() => f(...a), ms); }; };
