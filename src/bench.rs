//! The checked-in benchmark: the benchmark is the claim.
//!
//! `fixtures/benchmark.json` holds cases (a task plus candidates plus the
//! expected verdict and choice). `jev-route validate` runs the policy over
//! every case and exits non-zero on any miss. CI runs it on every push.

use crate::json;
use crate::policy::{self, Candidate, Policy, Task, Tier, Verdict};

pub struct Report {
    pub total: usize,
    pub passed: usize,
    pub failures: Vec<String>,
}

impl Report {
    pub fn exit_code(&self) -> i32 {
        if self.passed == self.total {
            0
        } else {
            3
        }
    }
}

fn verdict_from_str(s: &str) -> Option<Verdict> {
    match s {
        "PASS" => Some(Verdict::Pass),
        "ESCALATE" => Some(Verdict::Escalate),
        "BLOCK" => Some(Verdict::Block),
        _ => None,
    }
}

fn task_from(v: &json::Json) -> Result<Task, String> {
    let allowed = v
        .get("allowed_tiers")
        .and_then(|x| x.as_arr())
        .map(|arr| {
            arr.iter()
                .map(|t| {
                    Tier::parse(t.as_str().ok_or("tier entries must be strings")?)
                        .ok_or_else(|| "bad tier in case".to_string())
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    Ok(Task {
        id: v
            .get("id")
            .and_then(|x| x.as_str())
            .unwrap_or("case")
            .to_string(),
        complexity: v.get("complexity").and_then(|x| x.as_f64()).unwrap_or(0.5),
        risk: v.get("risk").and_then(|x| x.as_f64()).unwrap_or(0.5),
        needs_images: v
            .get("needs_images")
            .and_then(|x| x.as_bool())
            .unwrap_or(false),
        outward: v.get("outward").and_then(|x| x.as_bool()).unwrap_or(false),
        allowed_tiers: allowed,
    })
}

fn cands_from(v: &json::Json) -> Result<Vec<Candidate>, String> {
    let arr = v.as_arr().ok_or("candidates must be an array")?;
    arr.iter()
        .map(|c| {
            Ok(Candidate {
                name: c
                    .get("name")
                    .and_then(|x| x.as_str())
                    .ok_or("candidate needs name")?
                    .to_string(),
                tier: Tier::parse(
                    c.get("tier")
                        .and_then(|x| x.as_str())
                        .ok_or("candidate needs tier")?,
                )
                .ok_or("unknown tier")?,
                reliability: c.get("reliability").and_then(|x| x.as_f64()).unwrap_or(0.9),
                outward_ok: c
                    .get("outward_ok")
                    .and_then(|x| x.as_bool())
                    .unwrap_or(false),
            })
        })
        .collect()
}

pub fn run_fixture_text(text: &str) -> Result<Report, String> {
    let root = json::parse(text)?;
    let cases = root
        .get("cases")
        .and_then(|x| x.as_arr())
        .ok_or("missing \"cases\"")?;
    let policy = Policy::default();
    let mut failures = Vec::new();
    let mut passed = 0;

    for (idx, case) in cases.iter().enumerate() {
        let name = case
            .get("name")
            .and_then(|x| x.as_str())
            .unwrap_or("<unnamed>")
            .to_string();
        let task_v = case
            .get("task")
            .ok_or_else(|| format!("case {idx}: missing task"))?;
        let mut task = task_from(task_v)?;
        task.id = name.clone();
        let cands = cands_from(
            case.get("candidates")
                .ok_or_else(|| format!("case {idx}: missing candidates"))?,
        )?;
        let expected_verdict = verdict_from_str(
            case.get("expect_verdict")
                .and_then(|x| x.as_str())
                .ok_or("case needs expect_verdict")?,
        )
        .ok_or("bad expect_verdict")?;
        let expected_choice = case
            .get("expect_choice")
            .and_then(|x| x.as_str())
            .map(String::from);

        let d = policy::route(&task, &cands, &policy);
        let mut miss = Vec::new();
        if d.verdict != expected_verdict {
            miss.push(format!("verdict {} expected {expected_verdict}", d.verdict));
        }
        if let Some(ec) = &expected_choice {
            if d.choice.as_deref() != Some(ec.as_str()) {
                miss.push(format!("choice {:?} expected {ec:?}", d.choice));
            }
        }
        if miss.is_empty() {
            passed += 1;
        } else {
            failures.push(format!("{name}: {}", miss.join("; ")));
        }
    }

    Ok(Report {
        total: cases.len(),
        passed,
        failures,
    })
}

pub fn run_file(path: &str, quiet: bool) -> Result<Report, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    let report = run_fixture_text(&text)?;
    if !quiet || report.passed != report.total {
        println!("benchmark: {}/{} cases pass", report.passed, report.total);
        for f in &report.failures {
            println!("  FAIL {f}");
        }
    }
    Ok(report)
}
