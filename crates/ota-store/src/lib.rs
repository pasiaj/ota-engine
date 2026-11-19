//! # ota-store
//!
//! Plain-file persistence, readable without the program (see `docs/DATA.md`):
//!
//! ```text
//! data/
//!   meta.json               company, modules, people, classes, tools, LLM tiers
//!   days/D0001.json         one complete DayRecord per simulated day
//!   state/<name>.json       snapshots of tool state (ledger, assumptions, ...)
//!   state/private/<id>.json private per-human calibration profiles
//!   log/<name>.jsonl        append-only logs (kpis, decisions, messages, llm, human)
//!   run_status.json         background run progress
//! ```
//!
//! Writes are atomic (temp file + rename) so a reader never sees half a file.

use anyhow::{Context, Result};
use ota_core::{Day, DayRecord};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Store {
    pub root: PathBuf,
}

pub fn day_file_name(day: Day) -> String {
    format!("D{day:04}.json")
}

impl Store {
    pub fn open(root: impl AsRef<Path>) -> Result<Store> {
        let root = root.as_ref().to_path_buf();
        for d in ["days", "state", "state/private", "log"] {
            fs::create_dir_all(root.join(d)).with_context(|| format!("creating {}", root.join(d).display()))?;
        }
        Ok(Store { root })
    }

    fn write_atomic(&self, path: &Path, bytes: &[u8]) -> Result<()> {
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, bytes).with_context(|| format!("writing {}", tmp.display()))?;
        fs::rename(&tmp, path).with_context(|| format!("renaming to {}", path.display()))?;
        Ok(())
    }

    pub fn write_json<T: Serialize>(&self, rel: &str, v: &T) -> Result<()> {
        let p = self.root.join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent)?;
        }
        self.write_atomic(&p, &serde_json::to_vec(v)?)
    }

    pub fn read_json<T: DeserializeOwned>(&self, rel: &str) -> Result<Option<T>> {
        let p = self.root.join(rel);
        if !p.exists() {
            return Ok(None);
        }
        let s = fs::read(&p).with_context(|| format!("reading {}", p.display()))?;
        Ok(Some(serde_json::from_slice(&s).with_context(|| format!("parsing {}", p.display()))?))
    }

    pub fn read_raw(&self, rel: &str) -> Option<Vec<u8>> {
        fs::read(self.root.join(rel)).ok()
    }

    // ---- days

    pub fn write_day(&self, rec: &DayRecord) -> Result<()> {
        self.write_json(&format!("days/{}", day_file_name(rec.day)), rec)
    }

    pub fn read_day(&self, day: Day) -> Result<Option<DayRecord>> {
        self.read_json(&format!("days/{}", day_file_name(day)))
    }

    /// Raw bytes of a day file (the server streams these without re-serialising).
    pub fn read_day_raw(&self, day: Day) -> Option<Vec<u8>> {
        self.read_raw(&format!("days/{}", day_file_name(day)))
    }

    /// All stored days, ascending.
    pub fn days(&self) -> Vec<Day> {
        let mut v: Vec<Day> = fs::read_dir(self.root.join("days")).map(|rd| rd.flatten().filter_map(|e| {
            let n = e.file_name().to_string_lossy().to_string();
            n.strip_prefix('D')?.strip_suffix(".json")?.parse().ok()
        }).collect()).unwrap_or_default();
        v.sort();
        v
    }

    pub fn last_day(&self) -> Option<Day> {
        self.days().last().copied()
    }

    // ---- state snapshots

    pub fn save_state<T: Serialize>(&self, name: &str, v: &T) -> Result<()> {
        self.write_json(&format!("state/{name}.json"), v)
    }

    pub fn load_state<T: DeserializeOwned>(&self, name: &str) -> Result<Option<T>> {
        self.read_json(&format!("state/{name}.json"))
    }

    pub fn save_private<T: Serialize>(&self, person: &str, v: &T) -> Result<()> {
        self.write_json(&format!("state/private/{person}.json"), v)
    }

    // ---- append-only logs

    pub fn append<T: Serialize>(&self, log: &str, items: &[T]) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }
        let p = self.root.join("log").join(format!("{log}.jsonl"));
        let mut f = fs::OpenOptions::new().create(true).append(true).open(&p).with_context(|| format!("opening {}", p.display()))?;
        let mut buf = Vec::new();
        for it in items {
            serde_json::to_writer(&mut buf, it)?;
            buf.push(b'\n');
        }
        f.write_all(&buf)?;
        Ok(())
    }

    pub fn read_log(&self, log: &str) -> Vec<serde_json::Value> {
        fs::read_to_string(self.root.join("log").join(format!("{log}.jsonl")))
            .map(|s| s.lines().filter_map(|l| serde_json::from_str(l).ok()).collect())
            .unwrap_or_default()
    }

    /// Remove all days after `day` (and logs are truncated by day) — used to re-run.
    pub fn truncate_after(&self, day: Day) -> Result<()> {
        for d in self.days().into_iter().filter(|d| *d > day) {
            let _ = fs::remove_file(self.root.join("days").join(day_file_name(d)));
        }
        if let Ok(rd) = fs::read_dir(self.root.join("log")) {
            for e in rd.flatten() {
                let p = e.path();
                let Ok(s) = fs::read_to_string(&p) else { continue };
                let kept: Vec<&str> = s.lines().filter(|l| {
                    serde_json::from_str::<serde_json::Value>(l).ok()
                        .and_then(|v| v.get("day").and_then(|d| d.as_u64())).map(|d| d as Day <= day).unwrap_or(true)
                }).collect();
                let mut out = kept.join("\n");
                if !out.is_empty() { out.push('\n'); }
                self.write_atomic(&p, out.as_bytes())?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip() {
        let dir = std::env::temp_dir().join(format!("ota-store-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let s = Store::open(&dir).unwrap();
        let rec = DayRecord { day: 3, date: "2026-01-07".into(), ..Default::default() };
        s.write_day(&rec).unwrap();
        s.write_day(&DayRecord { day: 1, ..Default::default() }).unwrap();
        assert_eq!(s.days(), vec![1, 3]);
        assert_eq!(s.read_day(3).unwrap().unwrap().date, "2026-01-07");
        s.append("kpis", &[serde_json::json!({"day": 1, "x": 1}), serde_json::json!({"day": 3, "x": 2})]).unwrap();
        assert_eq!(s.read_log("kpis").len(), 2);
        s.truncate_after(2).unwrap();
        assert_eq!(s.days(), vec![1]);
        assert_eq!(s.read_log("kpis").len(), 1);
        s.save_state("x", &vec![1, 2]).unwrap();
        assert_eq!(s.load_state::<Vec<i32>>("x").unwrap().unwrap(), vec![1, 2]);
        let _ = fs::remove_dir_all(&dir);
    }
}
