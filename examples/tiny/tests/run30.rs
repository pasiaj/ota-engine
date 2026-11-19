use ota_core::*;
use ota_example_tiny::{company_dir, ToyWorld};
use ota_llm::LlmMode;
use ota_runtime::{Engine, EngineOptions};

fn engine(dir: &std::path::Path) -> Engine {
    let spec = ota_spec::load_company(company_dir()).unwrap_or_else(|e| panic!("{e}"));
    let mut opts = EngineOptions::new(dir);
    opts.llm_mode = LlmMode::Scripted;
    opts.checkpoint_every = 10;
    Engine::new(spec, Box::new(ToyWorld::default()), opts).unwrap()
}

#[test]
fn runs_thirty_days_and_resumes() {
    let dir = std::env::temp_dir().join(format!("ota-tiny-run30-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut e = engine(&dir);
    let mut seen = 0;
    e.run(30, Some(&mut |_r: &DayRecord, done: u32, _n: u32| seen = done)).unwrap();
    assert_eq!(seen, 30);
    assert_eq!(e.store.days().len(), 30);

    let recs: Vec<DayRecord> = (1..=30).map(|d| e.store.read_day(d).unwrap().unwrap()).collect();
    let all_decisions: Vec<&Decision> = recs.iter().flat_map(|r| r.decisions.iter()).collect();
    assert!(recs.iter().all(|r| !r.kpis.is_empty() && !r.signals.is_empty()), "every day has telemetry");
    assert!(recs.iter().all(|r| r.module_runs.iter().any(|m| m.module == "ua")), "ua thinks daily");
    assert!(!all_decisions.is_empty(), "decisions were made");
    assert!(all_decisions.iter().any(|d| d.tier == Tier::Rule), "rule-tier decisions");
    assert!(all_decisions.iter().all(|d| !d.accountable.is_empty()), "every decision names an accountable human");
    assert!(recs.iter().any(|r| !r.actions.is_empty()), "actions executed");
    assert!(recs.iter().any(|r| !r.human_actions.is_empty()), "stand-ins acted");
    assert!(recs.iter().any(|r| !r.deviations.is_empty()), "lattice flagged deviations");
    assert!(recs.iter().any(|r| r.brief.is_some()), "signal brief drafted");
    assert!(recs.iter().any(|r| !r.forecasts_resolved.is_empty()), "forecasts resolved");
    assert!(recs.iter().any(|r| r.decision_updates.iter().any(|d| d.outcome.is_some())), "outcomes sealed");
    assert!(recs.iter().any(|r| r.grid.is_some()), "grid assessed");
    assert!(recs.iter().any(|r| !r.premortems.is_empty()), "pre-mortems ran or were recorded as skipped");
    assert!(recs.iter().any(|r| r.module_runs.iter().any(|m| m.think.llm.is_some())), "escalations batched into an LLM call");
    assert!(recs.iter().any(|r| r.anomalies.len() > 0), "anomalies journaled");
    // layered rollout: sentinel off before day 10, brief before day 6
    assert!(recs[..9].iter().all(|r| !r.tools_enabled.contains(&"sentinel".to_string())));
    assert!(recs[..5].iter().all(|r| r.brief.is_none()));
    // records serialise as documented
    let v = serde_json::to_value(&recs[20]).unwrap();
    for k in ["day", "date", "kpis", "module_runs", "decisions", "messages", "metrics", "sentinel", "llm_calls"] {
        assert!(v.get(k).is_some(), "day record has `{k}`");
    }
    assert!(dir.join("meta.json").exists());
    assert!(dir.join("state/view.json").exists());
    assert!(!e.store.read_log("kpis").is_empty());
    drop(e);

    // resume from disk
    let mut e2 = engine(&dir);
    assert_eq!(e2.day(), 30);
    e2.run(2, None).unwrap();
    assert_eq!(e2.store.last_day(), Some(32));
    let _ = std::fs::remove_dir_all(&dir);
}
