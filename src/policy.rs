//! The routing policy: deterministic scoring of candidates against a task.
//!
//! This is the same policy shape that governs the RIG fleet: typed signals
//! produce a score per candidate, the best candidate wins only above a
//! confidence floor, and every decision names its fallback.

use std::fmt;

/// The tiers a candidate agent can sit on, cheapest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    Fast,
    Standard,
    Deep,
    Visual,
}

impl Tier {
    pub fn parse(s: &str) -> Option<Tier> {
        match s.to_ascii_lowercase().as_str() {
            "fast" => Some(Tier::Fast),
            "standard" => Some(Tier::Standard),
            "deep" => Some(Tier::Deep),
            "visual" => Some(Tier::Visual),
            _ => None,
        }
    }

    /// Baseline capability of the tier on a 0..=1 scale.
    pub fn capability(self) -> f64 {
        match self {
            Tier::Fast => 0.35,
            Tier::Standard => 0.60,
            Tier::Deep => 0.90,
            Tier::Visual => 0.70,
        }
    }

    /// Relative cost of the tier. Routing penalises overspend on cheap tasks.
    pub fn cost(self) -> f64 {
        match self {
            Tier::Fast => 0.10,
            Tier::Standard => 0.35,
            Tier::Deep => 1.00,
            Tier::Visual => 0.55,
        }
    }

    /// Whether this tier can see images at all.
    pub fn sees_images(self) -> bool {
        matches!(self, Tier::Standard | Tier::Deep | Tier::Visual)
    }
}

impl fmt::Display for Tier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Tier::Fast => "fast",
            Tier::Standard => "standard",
            Tier::Deep => "deep",
            Tier::Visual => "visual",
        };
        f.write_str(s)
    }
}

/// A candidate the router may pick.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub name: String,
    pub tier: Tier,
    /// Self-declared or measured reliability in 0..=1.
    pub reliability: f64,
    /// Whether this candidate is allowed to receive outward-facing work.
    pub outward_ok: bool,
}

/// The task being routed.
#[derive(Debug, Clone)]
pub struct Task {
    pub id: String,
    /// 0..=1. How much reasoning depth the task needs.
    pub complexity: f64,
    /// 0..=1. Cost of being wrong (a typo in a README vs. an outward email).
    pub risk: f64,
    pub needs_images: bool,
    /// The task produces something shown or sent outside the machine.
    pub outward: bool,
    /// Hard requirement: only these tiers may serve (empty = no constraint).
    pub allowed_tiers: Vec<Tier>,
}

impl Default for Task {
    fn default() -> Self {
        Task {
            id: "task".into(),
            complexity: 0.5,
            risk: 0.5,
            needs_images: false,
            outward: false,
            allowed_tiers: Vec::new(),
        }
    }
}

/// The verdict attached to a decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Confidence at or above the floor: dispatch the choice.
    Pass,
    /// Best score is below the floor: escalate to the fallback instead.
    Escalate,
    /// No eligible candidate at all: block and surface to a human.
    Block,
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Verdict::Pass => "PASS",
            Verdict::Escalate => "ESCALATE",
            Verdict::Block => "BLOCK",
        };
        f.write_str(s)
    }
}

/// The full routing decision. Everything a caller needs to act and to audit.
#[derive(Debug, Clone)]
pub struct Decision {
    pub task_id: String,
    pub choice: Option<String>,
    pub fallback: Option<String>,
    pub confidence: f64,
    pub verdict: Verdict,
    /// Per-candidate scores, for the receipt.
    pub scores: Vec<(String, f64)>,
    pub reasons: Vec<String>,
}

/// Policy knobs. Defaults are the fleet values in use since 2026-10.
#[derive(Debug, Clone)]
pub struct Policy {
    /// Minimum confidence to dispatch without escalation.
    pub confidence_floor: f64,
    /// Weight of capability match vs. cost in the score blend.
    pub capability_weight: f64,
    pub cost_weight: f64,
    /// Bonus for a candidate whose tier matches the task's complexity band.
    pub fit_bonus: f64,
}

impl Default for Policy {
    fn default() -> Self {
        Policy {
            confidence_floor: 0.55,
            capability_weight: 0.60,
            cost_weight: 0.20,
            fit_bonus: 0.20,
        }
    }
}

/// Score one candidate against one task. Deterministic, no hidden state.
pub fn score(task: &Task, c: &Candidate, policy: &Policy) -> (f64, Vec<String>) {
    let mut reasons = Vec::new();

    // Hard exclusions first: a disqualified candidate never gets a score.
    if !task.allowed_tiers.is_empty() && !task.allowed_tiers.contains(&c.tier) {
        reasons.push(format!("excluded: tier {} not in allowed set", c.tier));
        return (0.0, reasons);
    }
    if task.needs_images && !c.tier.sees_images() {
        reasons.push("excluded: task needs images, tier cannot see them".into());
        return (0.0, reasons);
    }
    if task.outward && !c.outward_ok {
        reasons.push("excluded: outward task, candidate not outward-approved".into());
        return (0.0, reasons);
    }

    // Capability must cover complexity; extra headroom is fine but not free.
    let capability_match = if c.tier.capability() >= task.complexity {
        reasons.push(format!(
            "capability {} covers complexity {:.2}",
            c.tier.capability(),
            task.complexity
        ));
        1.0
    } else {
        let shortfall = task.complexity - c.tier.capability();
        reasons.push(format!("capability shortfall {:.2}", shortfall));
        (1.0 - shortfall).max(0.0)
    };

    // Cost is penalised hardest when the task is cheap; risk buys back tolerance.
    let waste = (c.tier.cost() - task.complexity * task.complexity).max(0.0);
    let cost_score = 1.0 - waste * (1.0 - task.risk);
    reasons.push(format!(
        "cost score {:.3} (waste {:.3}, risk {:.2})",
        cost_score, waste, task.risk
    ));

    // Band fit: reward the candidate whose capability sits closest to what the
    // task actually needs. Right-sizing, not maxing out.
    let distance = (c.tier.capability() - task.complexity).abs();
    let fit = (1.0 - distance).max(0.0) * policy.fit_bonus;

    let score = policy.capability_weight * capability_match * c.reliability
        + policy.cost_weight * cost_score
        + fit;
    reasons.push(format!("band fit {:.3} (distance {:.2})", fit, distance));
    (score.min(1.0), reasons)
}

/// Route a task across candidates. The core entry point of the binary.
pub fn route(task: &Task, candidates: &[Candidate], policy: &Policy) -> Decision {
    let mut scored: Vec<(String, f64, Vec<String>)> = candidates
        .iter()
        .map(|c| {
            let (s, r) = score(task, c, policy);
            (c.name.clone(), s, r)
        })
        .collect();

    // Sort by score desc, then by name for determinism across ties.
    scored.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });

    let eligible: Vec<_> = scored.iter().filter(|(_, s, _)| *s > 0.0).collect();
    let scores: Vec<(String, f64)> = scored.iter().map(|(n, s, _)| (n.clone(), *s)).collect();

    if eligible.is_empty() {
        return Decision {
            task_id: task.id.clone(),
            choice: None,
            fallback: None,
            confidence: 0.0,
            verdict: Verdict::Block,
            scores,
            reasons: vec!["no eligible candidate".into()],
        };
    }

    let best = &eligible[0];
    let confidence = best.1;
    let fallback = eligible.get(1).map(|(n, _, _)| n.clone());
    let mut reasons = best.2.clone();

    let verdict = if confidence >= policy.confidence_floor {
        Verdict::Pass
    } else {
        reasons.push(format!(
            "confidence {:.3} below floor {:.2}",
            confidence, policy.confidence_floor
        ));
        Verdict::Escalate
    };

    Decision {
        task_id: task.id.clone(),
        choice: if verdict == Verdict::Pass {
            Some(best.0.clone())
        } else {
            fallback.clone()
        },
        fallback,
        confidence,
        verdict,
        scores,
        reasons,
    }
}
