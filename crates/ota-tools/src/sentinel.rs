//! Sentinel: computable signatures for the six pathogens, the AI failure modes, the
//! four signal classes (§13.2) and the action-side rules pilot limbo / big-bang (§13.9).
//! Signatures follow §6.1 as amended by §13.1. Epistemic corruption is detected by the
//! independent audit (a separate code path), not here.

use crate::calibration::Calibration;
use crate::history::History;
use crate::ledger::Ledger;
use ota_core::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MIN_HUMAN_MINUTES_HIGH_STAKES: f64 = 3.0;
pub const MIN_LAUNDERING_GAP_MIN: f64 = 30.0;
pub const HIGH_STAKES_EUR: f64 = 2000.0;
pub const REFIRE_DAYS: Day = 7;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Sentinel {
    pub findings: Vec<SentinelFinding>,
    pub last_fired: BTreeMap<String, Day>,
}

pub struct Input<'a> {
    pub day: Day,
    pub history: &'a History,
    pub ledger: &'a Ledger,
    pub calibration: &'a Calibration,
    pub pilot_limbo_n: usize,
}

type Hit = (String, BTreeMap<String, f64>, Vec<Id>, Severity);

fn stats(kv: &[(&str, f64)]) -> BTreeMap<String, f64> {
    kv.iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

fn rising(a: Option<f64>, b: Option<f64>, by: f64) -> bool {
    matches!((a, b), (Some(now), Some(before)) if now > before + by)
}

impl Sentinel {
    pub fn scan(&mut self, inp: &Input, ids: &mut IdGen) -> Vec<SentinelFinding> {
        let d = inp.day;
        let (r0, r1) = (d.saturating_sub(27), d); // recent 28 days
        let (p0, p1) = (d.saturating_sub(55), d.saturating_sub(28)); // previous 28 days
        let h = inp.history;
        let mut hits: Vec<(FindingKind, Hit)> = vec![];
        let mut add = |k: FindingKind, hit: Option<Hit>| if let Some(x) = hit { hits.push((k, x)) };

        // ---------------- pathogens
        add(FindingKind::Pathogen(Pathogen::Hubris), {
            let (c_now, c_before) = (h.mean(r0, r1, |a| a.ai_mean_confidence), h.mean(p0, p1, |a| a.ai_mean_confidence));
            let (b_now, b_before) = (inp.calibration.brier_window(r0, r1, Some(false)), inp.calibration.brier_window(p0, p1, Some(false)));
            (rising(c_now, c_before, 0.03) && matches!((b_now, b_before), (Some(n), Some(b)) if n >= b - 0.005)).then(|| (
                "mean stated confidence rising while Brier is flat or worse".into(),
                stats(&[("confidence_now", c_now.unwrap()), ("confidence_before", c_before.unwrap()), ("brier_now", b_now.unwrap()), ("brier_before", b_before.unwrap())]),
                vec![], Severity::Medium))
        });
        add(FindingKind::Pathogen(Pathogen::Complacency), {
            let stale: Vec<&crate::history::DevTrack> = h.devs.iter()
                .filter(|t| t.z.abs() >= 2.0 && t.day + 5 <= d && t.day + 28 >= d && t.acted_day.is_none() && t.dismissed_day.is_none()).collect();
            let overdue = h.days.last().map(|a| a.switches_overdue).unwrap_or(0);
            let aar_now = ratio(h.sum(r0, r1, |a| a.switches_sealed_on_time), h.sum(r0, r1, |a| a.switches_due));
            let aar_before = ratio(h.sum(p0, p1, |a| a.switches_sealed_on_time), h.sum(p0, p1, |a| a.switches_due));
            let aar_falling = matches!((aar_now, aar_before), (Some(n), Some(b)) if n < b - 0.15);
            (stale.len() >= 3 || overdue > 0 || aar_falling).then(|| (
                format!("{} deviation(s) ≥2σ without a decision for 5+ days; {overdue} switch check(s) overdue", stale.len()),
                stats(&[("unacted_deviations", stale.len() as f64), ("switches_overdue", overdue as f64), ("aar_completion", aar_now.unwrap_or(1.0))]),
                stale.iter().map(|t| t.id.clone()).take(10).collect(), Severity::Medium))
        });
        add(FindingKind::Pathogen(Pathogen::NormalisationOfDeviance), {
            let weeks: Vec<BTreeMap<String, usize>> = (0..3).map(|w| {
                let (a, b) = (d.saturating_sub(7 * (w + 1) - 1), d.saturating_sub(7 * w));
                let mut m = BTreeMap::new();
                for x in h.window(a, b) { for (k, v) in &x.overrides { *m.entry(k.clone()).or_insert(0) += v; } }
                m
            }).collect();
            weeks[0].iter().find(|(k, n)| **n >= 3 && **n > *weeks[1].get(*k).unwrap_or(&0) && weeks[1].get(*k).unwrap_or(&0) >= weeks[2].get(*k).unwrap_or(&0))
                .map(|(k, n)| (format!("accepted exceptions/overrides of kind `{k}` rising week over week"),
                               stats(&[("this_week", *n as f64), ("last_week", *weeks[1].get(k).unwrap_or(&0) as f64), ("two_weeks_ago", *weeks[2].get(k).unwrap_or(&0) as f64)]),
                               vec![], Severity::High))
        });
        add(FindingKind::Pathogen(Pathogen::Groupthink), {
            let ch = h.sum(r0, r1, |a| a.challenges);
            let ov = h.sum(r0, r1, |a| a.challenges_overridden);
            let sim = h.mean(r0, r1, |a| a.premortem_similarity);
            let rate = if ch > 0 { ov as f64 / ch as f64 } else { 0.0 };
            ((ch >= 4 && rate > 0.5) || sim.map(|s| s > 0.6).unwrap_or(false)).then(|| (
                format!("leaders override {:.0}% of adversarial challenges; pre-mortem similarity {:.2}", rate * 100.0, sim.unwrap_or(0.0)),
                stats(&[("override_rate", rate), ("challenges", ch as f64), ("premortem_similarity", sim.unwrap_or(0.0))]), vec![], Severity::Medium))
        });
        add(FindingKind::Pathogen(Pathogen::MisleadingIncentives), {
            let now = h.window(d.saturating_sub(6), d).filter_map(|a| a.coherence_gap).last();
            let before = h.window(p0, p1).filter_map(|a| a.coherence_gap).last();
            (now.map(|g| g > 0.25).unwrap_or(false) && rising(now, before, 0.02)).then(|| (
                "ledger spend mix diverging from the stated strategy (coherence gap rising)".into(),
                stats(&[("gap_now", now.unwrap()), ("gap_before", before.unwrap())]), vec![], Severity::Medium))
        });
        add(FindingKind::Pathogen(Pathogen::Inertia), {
            let crossed: Vec<Id> = inp.ledger.decisions.values()
                .filter(|x| x.outcome.as_ref().map(|o| o.stop_hit && o.sealed_day + 28 >= d).unwrap_or(false) && x.status != DecisionStatus::Reverted
                    && !inp.ledger.decisions.values().any(|y| y.when >= x.when && y.class == x.class && y.notes.iter().any(|n| n.contains(&format!("stops {}", x.id)))))
                .map(|x| x.id.clone()).collect();
            let stale_now = h.days.last().map(|a| a.stale_assumptions).unwrap_or(0);
            let stale_before = h.window(d.saturating_sub(30), d.saturating_sub(28)).map(|a| a.stale_assumptions).last().unwrap_or(0);
            (crossed.len() >= 2 || (stale_now > stale_before && stale_now >= 3)).then(|| (
                format!("{} stop signal(s) crossed without a stop decision; stale assumptions {stale_before}→{stale_now}", crossed.len()),
                stats(&[("stop_crossed", crossed.len() as f64), ("stale_now", stale_now as f64)]), crossed.into_iter().take(10).collect(), Severity::Medium))
        });

        // ---------------- AI failure modes
        add(FindingKind::FailureMode(AiFailureMode::Bypass), {
            let untraced = h.sum(d.saturating_sub(13), d, |a| a.untraced_human_actions);
            let routed: Vec<&crate::history::DevTrack> = h.devs.iter().filter(|t| t.routed && t.day + 14 >= d && t.day + 2 <= d).collect();
            let unheeded = routed.iter().filter(|t| t.acted_day.is_none()).count();
            let share = if routed.is_empty() { 0.0 } else { unheeded as f64 / routed.len() as f64 };
            (untraced > 0 || (routed.len() >= 10 && share > 0.8)).then(|| (
                format!("{untraced} human action(s) outside the ledger; {unheeded}/{} routed flags never acted on (14 d)", routed.len()),
                stats(&[("untraced", untraced as f64), ("unheeded_share", share)]), vec![], Severity::Medium))
        });
        add(FindingKind::FailureMode(AiFailureMode::Abdication), {
            let hs = h.sum(r0, r1, |a| a.high_stakes_approvals);
            let fast = h.sum(r0, r1, |a| a.fast_high_stakes_approvals);
            let copied = h.sum(r0, r1, |a| a.copied_reasons);
            ((hs >= 3 && fast as f64 / hs as f64 > 0.3) || copied >= 2).then(|| (
                format!("{fast}/{hs} high-stakes approvals under {MIN_HUMAN_MINUTES_HIGH_STAKES} min; {copied} reasons copy the AI rationale"),
                stats(&[("fast_share", if hs > 0 { fast as f64 / hs as f64 } else { 0.0 }), ("copied", copied as f64)]), vec![], Severity::High))
        });
        add(FindingKind::FailureMode(AiFailureMode::SyntheticCertainty), {
            let n = h.sum(d.saturating_sub(13), d, |a| a.single_source_high_conf);
            (n >= 3).then(|| (format!("{n} high-confidence decisions from a single source without triangulation (14 d)"),
                              stats(&[("count", n as f64)]), vec![], Severity::Medium))
        });
        add(FindingKind::FailureMode(AiFailureMode::AlertFatigue), {
            let routed = h.sum(d.saturating_sub(13), d, |a| a.alerts_routed);
            let sup = h.sum(d.saturating_sub(13), d, |a| a.alerts_suppressed);
            let lat_now = h.mean(d.saturating_sub(13), d, |a| a.ack_latency_days);
            let lat_before = h.mean(d.saturating_sub(41), d.saturating_sub(14), |a| a.ack_latency_days);
            let share = if routed + sup > 0 { sup as f64 / (routed + sup) as f64 } else { 0.0 };
            (share > 0.2 && sup >= 3 || rising(lat_now, lat_before, 1.0)).then(|| (
                format!("{sup} alerts over budget ({:.0}%); acknowledgement latency {:.1} d", share * 100.0, lat_now.unwrap_or(0.0)),
                stats(&[("suppressed_share", share), ("ack_latency", lat_now.unwrap_or(0.0))]), vec![], Severity::Medium))
        });
        add(FindingKind::FailureMode(AiFailureMode::AiEchoChamber), {
            let now = h.mean(d.saturating_sub(13), d, |a| a.external_share);
            let before = h.mean(p0, p1, |a| a.external_share);
            matches!((now, before), (Some(n), Some(b)) if b > 0.05 && n < b * 0.6).then(|| (
                "share of external-provenance signals in inputs falling".into(),
                stats(&[("external_now", now.unwrap()), ("external_before", before.unwrap())]), vec![], Severity::Medium))
        });
        add(FindingKind::FailureMode(AiFailureMode::DecisionLaundering), {
            let gaps: Vec<f64> = h.window(r0, r1).filter_map(|a| a.laundering_gap_min).collect();
            let mean = (!gaps.is_empty()).then(|| gaps.iter().sum::<f64>() / gaps.len() as f64);
            (gaps.len() >= 2 && mean.unwrap() < MIN_LAUNDERING_GAP_MIN).then(|| (
                format!("commit follows pre-mortem start after {:.0} min on average (< {MIN_LAUNDERING_GAP_MIN})", mean.unwrap()),
                stats(&[("mean_gap_min", mean.unwrap()), ("n_days", gaps.len() as f64)]), vec![], Severity::High))
        });
        add(FindingKind::FailureMode(AiFailureMode::AssumptionGraveyard), {
            let now = h.days.last().map(|a| a.stale_assumptions).unwrap_or(0);
            let before = h.window(d.saturating_sub(16), d.saturating_sub(14)).map(|a| a.stale_assumptions).last().unwrap_or(0);
            (now >= 2 && now > before).then(|| (format!("stale assumptions without archive/renew: {before} → {now}"),
                                                stats(&[("stale_now", now as f64), ("stale_before", before as f64)]), vec![], Severity::Medium))
        });
        add(FindingKind::FailureMode(AiFailureMode::GlassCockpit), {
            let s_now = ratio(h.sum(r0, r1, |a| a.manual_checks), h.sum(r0, r1, |a| a.human_actions));
            let s_before = ratio(h.sum(p0, p1, |a| a.manual_checks), h.sum(p0, p1, |a| a.human_actions));
            let n = h.sum(r0, r1, |a| a.human_actions);
            (n >= 10 && (matches!((s_now, s_before), (Some(a), Some(b)) if a < b * 0.6) || s_now.map(|s| s < 0.1).unwrap_or(false))).then(|| (
                format!("share of manual cycles (raw data read before the AI summary) {:.0}%", s_now.unwrap_or(0.0) * 100.0),
                stats(&[("manual_share_now", s_now.unwrap_or(0.0)), ("manual_share_before", s_before.unwrap_or(0.0))]), vec![], Severity::Medium))
        });
        add(FindingKind::FailureMode(AiFailureMode::LearningLoopInversion), {
            let rec = h.days.last().map(|a| a.double_loop_recurring).unwrap_or(0);
            let (en, eb) = (h.mean(d.saturating_sub(13), d, |a| a.external_share), h.mean(p0, p1, |a| a.external_share));
            let shift = matches!((en, eb), (Some(a), Some(b)) if (a - b).abs() > 0.15);
            (rec >= 3 && shift).then(|| ("double-loop recommendations keep recurring while external signals show a regime shift".into(),
                                         stats(&[("recurring", rec as f64), ("external_delta", en.unwrap() - eb.unwrap())]), vec![], Severity::High))
        });
        add(FindingKind::FailureMode(AiFailureMode::DiffusionOfAccountability), {
            let n = h.sum(d.saturating_sub(6), d, |a| a.no_accountable + a.diffuse_approvals);
            (n > 0).then(|| (format!("{n} decision(s) without a named accountable human or with ≥3 approvers and no owner"),
                             stats(&[("count", n as f64)]), vec![], Severity::High))
        });
        add(FindingKind::FailureMode(AiFailureMode::Homogenisation), {
            let s = h.mean(d.saturating_sub(6), d, |a| a.module_similarity);
            s.filter(|s| *s > 0.6).map(|s| (format!("module outputs converging (similarity {s:.2})"), stats(&[("similarity", s)]), vec![], Severity::Low))
        });
        add(FindingKind::FailureMode(AiFailureMode::CulturalBoundary), {
            let now = h.sum(r0, r1, |a| a.bad_news_by_humans);
            let before = h.sum(p0, p1, |a| a.bad_news_by_humans);
            (before >= 3 && now == 0).then(|| ("humans stopped raising bad news (cultural suppression)".into(),
                                               stats(&[("now", 0.0), ("before", before as f64)]), vec![], Severity::High))
        });

        // ---------------- signal classes
        add(FindingKind::SignalClass(SentinelClass::CalibrationDrift), {
            let (n, b) = (inp.calibration.brier_window(r0, r1, Some(false)), inp.calibration.brier_window(p0, p1, Some(false)));
            rising(n, b, 0.05).then(|| ("AI Brier score worsening".into(), stats(&[("brier_now", n.unwrap()), ("brier_before", b.unwrap())]), vec![], Severity::Medium))
        });
        add(FindingKind::SignalClass(SentinelClass::CapabilityDecay), {
            let m_now = h.mean(r0, r1, |a| (a.human_actions > 0).then(|| a.human_minutes / a.human_actions as f64));
            let m_before = h.mean(p0, p1, |a| (a.human_actions > 0).then(|| a.human_minutes / a.human_actions as f64));
            matches!((m_now, m_before), (Some(a), Some(b)) if a < b * 0.6).then(|| (
                "minutes humans spend per review falling sharply".into(),
                stats(&[("min_per_action_now", m_now.unwrap()), ("min_per_action_before", m_before.unwrap())]), vec![], Severity::Medium))
        });
        add(FindingKind::SignalClass(SentinelClass::FallingDissent), {
            let dn = ratio(h.sum(r0, r1, |a| a.challenges + a.human_rejections), h.sum(r0, r1, |a| a.decisions));
            let db = ratio(h.sum(p0, p1, |a| a.challenges + a.human_rejections), h.sum(p0, p1, |a| a.decisions));
            matches!((dn, db), (Some(a), Some(b)) if b > 0.05 && a < b * 0.5).then(|| (
                "dissent per decision (challenges + rejections) halved".into(),
                stats(&[("dissent_now", dn.unwrap()), ("dissent_before", db.unwrap())]), vec![], Severity::Medium))
        });
        add(FindingKind::SignalClass(SentinelClass::DecayingMetrics), {
            let n = h.days.last().map(|a| a.kpis_decaying).unwrap_or(0);
            (n >= 2).then(|| (format!("{n} target KPIs trending the wrong way over 28 days"), stats(&[("kpis", n as f64)]), vec![], Severity::Low))
        });

        // ---------------- action-side rules
        let (limbo, bigbang) = action_rules(inp);
        add(FindingKind::Rule("pilot_limbo".into()), limbo);
        add(FindingKind::Rule("big_bang".into()), bigbang);
        add(FindingKind::Rule("missing_action_elements".into()), {
            let n = h.days.last().map(|a| a.missing_action_elements).unwrap_or(0);
            (n > 0).then(|| (format!("{n} test/launch/scale decision(s) missing one of the four action elements"), stats(&[("count", n as f64)]), vec![], Severity::Low))
        });

        let mut out = vec![];
        for (kind, (signature, st, evidence, severity)) in hits {
            let key = serde_json::to_string(&kind).unwrap_or_default();
            if self.last_fired.get(&key).map(|l| *l + REFIRE_DAYS > d).unwrap_or(false) {
                continue;
            }
            self.last_fired.insert(key, d);
            let f = SentinelFinding { id: ids.next("SN"), day: d, kind, signature, stats: st, evidence, severity, status: FindingStatus::Open };
            self.findings.push(f.clone());
            out.push(f);
        }
        out
    }
}

fn ratio(a: usize, b: usize) -> Option<f64> {
    (b > 0).then(|| a as f64 / b as f64)
}

fn action_rules(inp: &Input) -> (Option<Hit>, Option<Hit>) {
    let d = inp.day;
    let mut by_module: BTreeMap<&str, Vec<&Decision>> = BTreeMap::new();
    for x in inp.ledger.decisions.values() {
        by_module.entry(x.module.as_str()).or_default().push(x);
    }
    let mut limbo = None;
    let mut bigbang = None;
    for (m, ds) in by_module {
        let last_gokill = ds.iter().filter(|x| x.kind == DecisionKind::GoKill).map(|x| x.when).max().unwrap_or(0);
        let open_tests: Vec<Id> = ds.iter().filter(|x| x.kind == DecisionKind::Test && x.when > last_gokill
            && !matches!(x.status, DecisionStatus::Rejected | DecisionStatus::Blocked | DecisionStatus::Deferred)).map(|x| x.id.clone()).collect();
        if open_tests.len() > inp.pilot_limbo_n && limbo.is_none() {
            limbo = Some((format!("{m}: {} tests without a go/kill decision", open_tests.len()),
                          stats(&[("open_tests", open_tests.len() as f64)]), open_tests, Severity::Medium));
        }
        for s in ds.iter().filter(|x| x.kind == DecisionKind::Scale && x.when == d) {
            let tested = ds.iter().any(|x| x.kind == DecisionKind::Test && x.when + 90 >= d && x.when <= d);
            if !tested && bigbang.is_none() {
                bigbang = Some((format!("{m}: scale-up {} without a test in the prior 90 days", s.id), stats(&[]), vec![s.id.clone()], Severity::High));
            }
        }
    }
    (limbo, bigbang)
}
