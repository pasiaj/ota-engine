//! Lenient Markdown helpers: YAML front-matter, `- key: value` bullets, pipe tables and
//! `## heading` sections.

use std::collections::BTreeMap;

/// Split `---\nyaml\n---\nbody`. Returns (yaml, body). No front-matter → (None, whole text).
pub fn front_matter(src: &str) -> (Option<String>, String) {
    let s = src.trim_start_matches('\u{feff}');
    let mut lines = s.lines();
    if lines.next().map(str::trim) != Some("---") {
        return (None, s.to_string());
    }
    let mut yaml = String::new();
    let mut body = String::new();
    let mut in_yaml = true;
    for l in lines {
        if in_yaml && (l.trim() == "---" || l.trim() == "...") {
            in_yaml = false;
            continue;
        }
        if in_yaml {
            yaml.push_str(l);
            yaml.push('\n');
        } else {
            body.push_str(l);
            body.push('\n');
        }
    }
    if in_yaml {
        // unterminated: treat everything as body
        return (None, s.to_string());
    }
    (Some(yaml), body)
}

/// A `##`-level section (any level ≥ 2 counts as a section start at that level).
#[derive(Debug, Clone)]
pub struct Section {
    pub level: usize,
    pub title: String,
    pub body: String,
}

/// Split a document into sections at headings of exactly `level` (e.g. 2 for `##`).
/// Text before the first heading is returned as a section with an empty title.
pub fn sections(src: &str, level: usize) -> Vec<Section> {
    let prefix = "#".repeat(level);
    let mut out = vec![Section { level, title: String::new(), body: String::new() }];
    let mut in_code = false;
    for l in src.lines() {
        if l.trim_start().starts_with("```") {
            in_code = !in_code;
        }
        let t = l.trim_start();
        if !in_code && t.starts_with(&prefix) && t[level..].starts_with(' ') {
            out.push(Section { level, title: t[level..].trim().to_string(), body: String::new() });
        } else {
            let cur = out.last_mut().unwrap();
            cur.body.push_str(l);
            cur.body.push('\n');
        }
    }
    out
}

/// Remove a trailing ` # comment` (outside quotes/backticks).
pub fn strip_comment(v: &str) -> &str {
    let mut q: Option<char> = None;
    let b = v.as_bytes();
    for (i, c) in v.char_indices() {
        match (c, q) {
            ('"' | '\'' | '`', None) => q = Some(c),
            (c2, Some(qq)) if c2 == qq => q = None,
            ('#', None) if i > 0 && (b[i - 1] == b' ' || b[i - 1] == b'\t') => return v[..i].trim_end(),
            _ => {}
        }
    }
    v
}

pub fn unquote(v: &str) -> String {
    let t = v.trim();
    for q in ['"', '\'', '`'] {
        if t.len() >= 2 && t.starts_with(q) && t.ends_with(q) {
            return t[1..t.len() - 1].to_string();
        }
    }
    t.to_string()
}

/// `- key: value` bullets (also `* key: value`). Keys are normalised to snake_case.
/// Repeated keys are kept in order. Bullets without a colon are returned under "".
pub fn bullets(src: &str) -> Vec<(String, String)> {
    let mut out = vec![];
    for l in src.lines() {
        let t = l.trim_start();
        let Some(rest) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) else { continue };
        let rest = rest.trim();
        // `**key**: value` emphasis is tolerated
        let rest_owned = rest.replacen("**", "", 2);
        match rest_owned.split_once(':') {
            Some((k, v)) if !k.trim().is_empty() && k.split_whitespace().count() <= 3 && !k.contains(['(', '<', '>', '=']) => {
                out.push((norm_key(k), unquote(strip_comment(v.trim()))));
            }
            _ => out.push((String::new(), unquote(strip_comment(&rest_owned)))),
        }
    }
    out
}

pub fn norm_key(k: &str) -> String {
    k.trim().trim_matches('`').to_ascii_lowercase().replace(['-', ' '], "_")
}

/// Parse `[a, b]`, `a, b`, or `a` into a list.
pub fn list(v: &str) -> Vec<String> {
    let t = v.trim().trim_start_matches('[').trim_end_matches(']');
    t.split(',').map(unquote).filter(|s| !s.is_empty()).collect()
}

pub fn parse_bool(v: &str) -> Option<bool> {
    match v.trim().to_ascii_lowercase().as_str() {
        "true" | "yes" | "y" | "1" | "on" => Some(true),
        "false" | "no" | "n" | "0" | "off" | "" => Some(false),
        _ => None,
    }
}

/// Split a table row on `|`, honouring `\|` escapes and backtick spans.
fn split_row(line: &str) -> Vec<String> {
    let t = line.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').unwrap_or(t);
    let (mut cells, mut cur, mut tick, mut esc) = (vec![], String::new(), false, false);
    for c in t.chars() {
        if esc {
            cur.push(c);
            esc = false;
            continue;
        }
        match c {
            '\\' => esc = true,
            '`' => { tick = !tick; cur.push(c); }
            '|' if !tick => cells.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    cells.push(cur);
    cells.into_iter().map(|c| c.trim().to_string()).collect()
}

fn is_separator(cells: &[String]) -> bool {
    !cells.is_empty() && cells.iter().all(|c| !c.is_empty() && c.chars().all(|ch| matches!(ch, '-' | ':' | ' ')))
}

/// All pipe tables in `src`, each as rows of (normalised header → cell).
pub fn tables(src: &str) -> Vec<Vec<BTreeMap<String, String>>> {
    let mut out = vec![];
    let lines: Vec<&str> = src.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        if lines[i].trim_start().starts_with('|') && i + 1 < lines.len() && is_separator(&split_row(lines[i + 1])) {
            let header: Vec<String> = split_row(lines[i]).iter().map(|h| norm_key(&h.replace('*', ""))).collect();
            i += 2;
            let mut rows = vec![];
            while i < lines.len() && lines[i].trim_start().starts_with('|') {
                let cells = split_row(lines[i]);
                let mut m = BTreeMap::new();
                for (h, c) in header.iter().zip(cells.into_iter()) {
                    m.insert(h.clone(), c);
                }
                rows.push(m);
                i += 1;
            }
            out.push(rows);
        } else {
            i += 1;
        }
    }
    out
}

/// Numbered list items (`1. text`).
pub fn numbered(src: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for l in src.lines() {
        let t = l.trim_start();
        let digits: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() && t[digits.len()..].starts_with(". ") {
            out.push(t[digits.len() + 2..].trim().to_string());
        } else if !t.is_empty() && l.starts_with("   ") && !out.is_empty() {
            let last = out.last_mut().unwrap();
            last.push(' ');
            last.push_str(t);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fm_and_tables() {
        let (y, b) = front_matter("---\nid: x\n---\n# T\n");
        assert_eq!(y.unwrap(), "id: x\n");
        assert_eq!(b, "# T\n");
        let t = tables("| When | then |\n|---|--:|\n| `a || b` | x \\| y |\n| c | d |\n");
        assert_eq!(t[0][0]["when"], "`a || b`");
        assert_eq!(t[0][0]["then"], "x | y");
        assert_eq!(t[0].len(), 2);
        let bl = bullets("- reversibility: reversible   # note\n- forecast: \"D7 ≥ x # not comment\"\n- plain item");
        assert_eq!(bl[0], ("reversibility".into(), "reversible".into()));
        assert_eq!(bl[1].1, "D7 ≥ x # not comment");
        assert_eq!(list("[a, b]"), vec!["a", "b"]);
        assert_eq!(numbered("1. one\n   more\n2. two"), vec!["one more", "two"]);
    }
}
