//! `ota-check <company_dir>`: load and validate a company spec, print a summary,
//! warnings and all errors. Exit code 1 on errors.

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    match ota_spec::load_company(&dir) {
        Ok(s) => {
            println!("{} ({}) — day 1 = {}", s.company.name, s.company.id, s.company.start_date);
            println!("  modules: {}  people: {}  decision classes: {}  assumptions: {}  kpis: {}  okrs: {}  scenarios: {}  claims: {}",
                     s.modules.len(), s.people.len(), s.classes.len(), s.assumptions.len(), s.kpis.len(), s.okrs.len(), s.scenarios.len(), s.claims.len());
            for t in &s.toolkit {
                println!("  tool {:<12} from day {}", t.id, t.enabled_from);
            }
            for w in &s.warnings {
                println!("warning: {w}");
            }
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
