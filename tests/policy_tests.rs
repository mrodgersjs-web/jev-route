//! Integration tests: the policy contract and the checked-in benchmark.

use jev_route::policy::{self, Candidate, Policy, Task, Tier, Verdict};

fn cand(name: &str, tier: Tier, rel: f64, outward_ok: bool) -> Candidate {
    Candidate {
        name: name.into(),
        tier,
        reliability: rel,
        outward_ok,
    }
}

#[test]
fn right_sizing_beats_muscle() {
    let task = Task {
        id: "t".into(),
        complexity: 0.2,
        risk: 0.1,
        ..Task::default()
    };
    let cands = vec![
        cand("workhorse", Tier::Deep, 0.97, true),
        cand("scout", Tier::Fast, 0.9, false),
    ];
    let d = policy::route(&task, &cands, &Policy::default());
    assert_eq!(d.verdict, Verdict::Pass);
    assert_eq!(d.choice.as_deref(), Some("scout"));
}

#[test]
fn outward_tasks_gate_on_approval() {
    let task = Task {
        id: "t".into(),
        complexity: 0.5,
        risk: 0.9,
        outward: true,
        ..Task::default()
    };
    let cands = vec![
        cand("scout", Tier::Fast, 0.95, false),
        cand("workhorse", Tier::Deep, 0.97, true),
    ];
    let d = policy::route(&task, &cands, &Policy::default());
    assert_eq!(d.choice.as_deref(), Some("workhorse"));
}

#[test]
fn blind_tier_cannot_take_image_work() {
    let task = Task {
        id: "t".into(),
        complexity: 0.6,
        risk: 0.4,
        needs_images: true,
        ..Task::default()
    };
    let cands = vec![cand("scout", Tier::Fast, 1.0, true)];
    let d = policy::route(&task, &cands, &Policy::default());
    assert_eq!(d.verdict, Verdict::Block);
}

#[test]
fn weak_candidate_escalates_instead_of_passing() {
    let task = Task {
        id: "t".into(),
        complexity: 0.5,
        risk: 0.5,
        ..Task::default()
    };
    let cands = vec![cand("flaky", Tier::Standard, 0.25, true)];
    let d = policy::route(&task, &cands, &Policy::default());
    assert_eq!(d.verdict, Verdict::Escalate);
    assert!(d.confidence < 0.55);
}

#[test]
fn ties_break_deterministically_by_name() {
    let task = Task {
        id: "t".into(),
        complexity: 0.45,
        risk: 0.5,
        ..Task::default()
    };
    let cands = vec![
        cand("zeta", Tier::Standard, 0.9, true),
        cand("alpha", Tier::Standard, 0.9, true),
    ];
    let d = policy::route(&task, &cands, &Policy::default());
    assert_eq!(d.choice.as_deref(), Some("alpha"));
    let d2 = policy::route(&task, &cands, &Policy::default());
    assert_eq!(d.scores, d2.scores, "routing must be a pure function");
}

#[test]
fn allowed_tiers_is_a_hard_gate() {
    let task = Task {
        id: "t".into(),
        complexity: 0.5,
        risk: 0.5,
        allowed_tiers: vec![Tier::Visual],
        ..Task::default()
    };
    let cands = vec![cand("scout", Tier::Fast, 0.9, true)];
    let d = policy::route(&task, &cands, &Policy::default());
    assert_eq!(d.verdict, Verdict::Block);
}

#[test]
fn benchmark_fixture_ships_green() {
    let report =
        jev_route::bench::run_file("fixtures/benchmark.json", true).expect("fixture parses");
    assert_eq!(report.failures, Vec::<String>::new());
    assert!(report.total >= 7);
    assert_eq!(report.passed, report.total);
}
