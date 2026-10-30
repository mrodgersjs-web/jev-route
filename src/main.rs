//! jev-route: the routing policy that governs the RIG fleet as a standalone
//! binary. Task in, choice + confidence + fallback verdict out.
//!
//!   jev-route route    --task task.json --candidates candidates.json [--policy policy.json] [--json]
//!   jev-route bench    [--fixture fixtures/benchmark.json]
//!   jev-route validate --fixture fixtures/benchmark.json
//!
//! Exit codes: 0 dispatch, 3 escalate, 4 block (benchmark: 0 all pass, 3 any fail).

use jev_route::{bench, json, policy};

use policy::{Candidate, Policy, Task, Tier, Verdict};

fn usage() -> ! {
    eprintln!("usage:");
    eprintln!("  jev-route route  --task TASK.json --candidates CANDIDATES.json [--policy POLICY.json] [--json]");
    eprintln!("  jev-route bench  [--fixture fixtures/benchmark.json]");
    eprintln!("  jev-route validate --fixture fixtures/benchmark.json");
    std::process::exit(2);
}

fn read(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))
}

fn parse_task(v: &json::Json) -> Result<Task, String> {
    let num =
        |key: &str, default: f64| -> f64 { v.get(key).and_then(|x| x.as_f64()).unwrap_or(default) };
    let flag = |key: &str| v.get(key).and_then(|x| x.as_bool()).unwrap_or(false);
    let mut allowed_tiers = Vec::new();
    if let Some(arr) = v.get("allowed_tiers").and_then(|x| x.as_arr()) {
        for item in arr {
            let s = item
                .as_str()
                .ok_or("allowed_tiers entries must be strings")?;
            allowed_tiers.push(Tier::parse(s).ok_or(format!("unknown tier {s}"))?);
        }
    }
    Ok(Task {
        id: v
            .get("id")
            .and_then(|x| x.as_str())
            .unwrap_or("task")
            .to_string(),
        complexity: num("complexity", 0.5),
        risk: num("risk", 0.5),
        needs_images: flag("needs_images"),
        outward: flag("outward"),
        allowed_tiers,
    })
}

fn parse_candidates(v: &json::Json) -> Result<Vec<Candidate>, String> {
    let arr = v
        .get("candidates")
        .and_then(|x| x.as_arr())
        .ok_or("missing \"candidates\" array")?;
    let mut out = Vec::new();
    for item in arr {
        let name = item
            .get("name")
            .and_then(|x| x.as_str())
            .ok_or("candidate needs a name")?;
        let tier_s = item
            .get("tier")
            .and_then(|x| x.as_str())
            .ok_or("candidate needs a tier")?;
        let tier = Tier::parse(tier_s).ok_or(format!("unknown tier {tier_s}"))?;
        let reliability = item
            .get("reliability")
            .and_then(|x| x.as_f64())
            .unwrap_or(0.9);
        let outward_ok = item
            .get("outward_ok")
            .and_then(|x| x.as_bool())
            .unwrap_or(false);
        out.push(Candidate {
            name: name.to_string(),
            tier,
            reliability,
            outward_ok,
        });
    }
    Ok(out)
}

fn parse_policy(v: &json::Json) -> Policy {
    let mut p = Policy::default();
    if let Some(x) = v.get("confidence_floor").and_then(|x| x.as_f64()) {
        p.confidence_floor = x;
    }
    if let Some(x) = v.get("capability_weight").and_then(|x| x.as_f64()) {
        p.capability_weight = x;
    }
    if let Some(x) = v.get("cost_weight").and_then(|x| x.as_f64()) {
        p.cost_weight = x;
    }
    if let Some(x) = v.get("fit_bonus").and_then(|x| x.as_f64()) {
        p.fit_bonus = x;
    }
    p
}

fn decision_json(d: &policy::Decision) -> String {
    let scores = d
        .scores
        .iter()
        .map(|(n, s)| format!("    \"{}\": {:.4}", n, s))
        .collect::<Vec<_>>()
        .join(",\n");
    let reasons = d
        .reasons
        .iter()
        .map(|r| format!("    {:?}", r))
        .collect::<Vec<_>>()
        .join(",\n");
    let opt = |o: &Option<String>| {
        o.as_deref()
            .map(|s| format!("\"{s}\""))
            .unwrap_or("null".into())
    };
    format!("{{\n  \"task_id\": {:?},\n  \"choice\": {},\n  \"fallback\": {},\n  \"confidence\": {:.4},\n  \"verdict\": \"{}\",\n  \"scores\": {{\n{}\n  }},\n  \"reasons\": [\n{}\n  ]\n}}",
        d.task_id, opt(&d.choice), opt(&d.fallback), d.confidence, d.verdict, scores, reasons)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first() else { usage() };

    match cmd.as_str() {
        "route" => {
            let mut task_path = None;
            let mut cand_path = None;
            let mut policy_path = None;
            let mut as_json = false;
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--task" => {
                        task_path = args.get(i + 1).cloned();
                        i += 2;
                    }
                    "--candidates" => {
                        cand_path = args.get(i + 1).cloned();
                        i += 2;
                    }
                    "--policy" => {
                        policy_path = args.get(i + 1).cloned();
                        i += 2;
                    }
                    "--json" => {
                        as_json = true;
                        i += 1;
                    }
                    _ => usage(),
                }
            }
            let (Some(t), Some(c)) = (task_path, cand_path) else {
                usage()
            };
            let task = match read(&t)
                .and_then(|s| json::parse(&s).map_err(|e| format!("{t}: {e}")))
                .and_then(|v| parse_task(&v))
            {
                Ok(x) => x,
                Err(e) => {
                    eprintln!("jev-route: {e}");
                    std::process::exit(2);
                }
            };
            let cands = match read(&c)
                .and_then(|s| json::parse(&s).map_err(|e| format!("{c}: {e}")))
                .and_then(|v| parse_candidates(&v))
            {
                Ok(x) => x,
                Err(e) => {
                    eprintln!("jev-route: {e}");
                    std::process::exit(2);
                }
            };
            let pol = match &policy_path {
                Some(p) => {
                    match read(p).and_then(|s| json::parse(&s).map_err(|e| format!("{p}: {e}"))) {
                        Ok(v) => parse_policy(&v),
                        Err(e) => {
                            eprintln!("jev-route: {e}");
                            std::process::exit(2);
                        }
                    }
                }
                None => Policy::default(),
            };
            let d = policy::route(&task, &cands, &pol);
            if as_json {
                println!("{}", decision_json(&d));
            } else {
                println!("task     {}", d.task_id);
                println!("verdict  {}", d.verdict);
                println!(
                    "choice   {}",
                    d.choice.clone().unwrap_or_else(|| "-".into())
                );
                println!(
                    "fallback {}",
                    d.fallback.clone().unwrap_or_else(|| "-".into())
                );
                println!("conf     {:.3}", d.confidence);
                for (n, s) in &d.scores {
                    println!("  {n}: {s:.4}");
                }
                for r in &d.reasons {
                    println!("  why: {r}");
                }
            }
            std::process::exit(match d.verdict {
                Verdict::Pass => 0,
                Verdict::Escalate => 3,
                Verdict::Block => 4,
            });
        }
        "bench" | "validate" => {
            let mut fixture = "fixtures/benchmark.json".to_string();
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--fixture" => {
                        fixture = args.get(i + 1).cloned().unwrap_or(fixture);
                        i += 2;
                    }
                    _ => usage(),
                }
            }
            let quiet = cmd == "bench";
            match bench::run_file(&fixture, quiet) {
                Ok(report) => std::process::exit(report.exit_code()),
                Err(e) => {
                    eprintln!("jev-route: {e}");
                    std::process::exit(2);
                }
            }
        }
        "--version" | "-V" => println!("jev-route {}", env!("CARGO_PKG_VERSION")),
        _ => usage(),
    }
}
