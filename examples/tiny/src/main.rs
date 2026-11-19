//! `ota-example-tiny run <days> [data_dir]` — run the toy company offline.
//! `ota-example-tiny serve [data_dir]` — serve it with the OTA API (see README).

use ota_example_tiny::{company_dir, ToyWorld};
use ota_runtime::{Engine, EngineOptions};

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("run");
    match cmd {
        "run" => {
            let n: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(30);
            let dir = args.get(3).cloned().unwrap_or_else(|| "data".into());
            let spec = ota_spec::load_company(company_dir()).map_err(|e| anyhow::anyhow!("{e}"))?;
            for w in &spec.warnings { eprintln!("warning: {w}"); }
            let mut e = Engine::new(spec, Box::new(ToyWorld::default()), EngineOptions::new(dir))?;
            e.run(n, Some(&mut |r, done, total| {
                eprintln!("[{done}/{total}] D{:04} {} decisions={} actions={} sentinel={} cost=${:.4}", r.day, r.date,
                          r.decisions.len(), r.actions.len(), r.sentinel.len(), r.metrics.llm_cost_usd);
            }))?;
        }
        other => anyhow::bail!("unknown command `{other}` (use: run <days> [dir])"),
    }
    Ok(())
}
