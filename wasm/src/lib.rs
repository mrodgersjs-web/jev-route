//! Raw-ABI WebAssembly surface for the jev-route policy.
//!
//! No wasm-bindgen, no JS glue generator: four exported functions and one
//! JSON string in memory. The JS wrapper in `js/index.js` is ~40 lines and
//! the whole contract is auditable.
//!
//!   alloc(len) -> ptr                 reserve room for an input string
//!   route(ptr, len) -> out_ptr        run the policy; result JSON is
//!                                     result_len() bytes at out_ptr
//!   result_len() -> len               byte length of the last result
//!   dealloc(ptr, len)                 free an input or result buffer
//!
//! Input JSON: {"task": {...}, "candidates": [...], "policy": {...optional}}
//! Output JSON: the Decision with "ok": true, or {"ok": false, "error": "..."}.
//! Panics are caught and reported as {"ok": false, "error": "panic: ..."}
//! instead of trapping, so a defect degrades to a readable error.

use jev_route::json;
use jev_route::policy::{self, Candidate, Policy, Task, Tier};
use std::cell::RefCell;

thread_local! {
    static RESULT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

#[no_mangle]
pub extern "C" fn alloc(len: usize) -> *mut u8 {
    let mut buf: Vec<u8> = Vec::with_capacity(len);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

#[no_mangle]
pub extern "C" fn dealloc(ptr: *mut u8, len: usize) {
    if !ptr.is_null() {
        unsafe { drop(Vec::from_raw_parts(ptr, len, len)) };
    }
}

#[no_mangle]
pub extern "C" fn result_len() -> usize {
    RESULT.with(|r| r.borrow().len())
}

#[no_mangle]
pub extern "C" fn route(ptr: *const u8, len: usize) -> *const u8 {
    let input = unsafe { Vec::from_raw_parts(ptr as *mut u8, len, len) };
    let out = route_inner(&input);
    drop(input);
    RESULT.with(|r| *r.borrow_mut() = out);
    RESULT.with(|r| r.borrow().as_ptr())
}

fn route_inner(input: &[u8]) -> Vec<u8> {
    let fail = |e: String| format!("{{\"ok\":false,\"error\":{}}}", json_str(&e)).into_bytes();
    let text = match std::str::from_utf8(input) {
        Ok(t) => t,
        Err(e) => return fail(format!("input is not valid UTF-8: {e}")),
    };
    let parsed = match json::parse(text) {
        Ok(v) => v,
        Err(e) => return fail(format!("input is not valid JSON: {e}")),
    };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(parsed))) {
        Ok(Ok(json)) => json.into_bytes(),
        Ok(Err(e)) => fail(e),
        Err(p) => fail(format!("panic: {}", panic_message(&p))),
    }
}

fn panic_message(p: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = p.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = p.downcast_ref::<String>() {
        s.clone()
    } else {
        "panic with non-string payload".to_string()
    }
}

fn run(v: json::Json) -> Result<String, String> {
    let task_v = v.get("task").ok_or("missing \"task\" object")?;
    let task = parse_task(task_v)?;
    let cands_v = v.get("candidates").ok_or("missing \"candidates\" array")?;
    let candidates = parse_candidates(cands_v)?;
    let policy = v.get("policy").map(parse_policy).unwrap_or_default();
    let d = policy::route(&task, &candidates, &policy);
    Ok(decision_json(&d))
}

fn parse_task(v: &json::Json) -> Result<Task, String> {
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
        complexity: v.get("complexity").and_then(|x| x.as_f64()).unwrap_or(0.5),
        risk: v.get("risk").and_then(|x| x.as_f64()).unwrap_or(0.5),
        needs_images: v
            .get("needs_images")
            .and_then(|x| x.as_bool())
            .unwrap_or(false),
        outward: v.get("outward").and_then(|x| x.as_bool()).unwrap_or(false),
        allowed_tiers,
    })
}

fn parse_candidates(v: &json::Json) -> Result<Vec<Candidate>, String> {
    let arr = v.as_arr().ok_or("\"candidates\" must be an array")?;
    arr.iter()
        .map(|c| {
            let name = c
                .get("name")
                .and_then(|x| x.as_str())
                .ok_or("candidate needs a name")?;
            let tier_s = c
                .get("tier")
                .and_then(|x| x.as_str())
                .ok_or("candidate needs a tier")?;
            let tier = Tier::parse(tier_s).ok_or(format!("unknown tier {tier_s}"))?;
            Ok(Candidate {
                name: name.to_string(),
                tier,
                reliability: c.get("reliability").and_then(|x| x.as_f64()).unwrap_or(0.9),
                outward_ok: c
                    .get("outward_ok")
                    .and_then(|x| x.as_bool())
                    .unwrap_or(false),
            })
        })
        .collect()
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
    let scores: Vec<String> = d
        .scores
        .iter()
        .map(|(n, s)| format!("{}:{}", json_str(n), round4(*s)))
        .collect();
    let reasons: Vec<String> = d.reasons.iter().map(|r| json_str(r)).collect();
    format!(
        "{{\"ok\":true,\"task_id\":{},\"choice\":{},\"fallback\":{},\"confidence\":{},\"verdict\":\"{}\",\"scores\":{{{}}},\"reasons\":[{}]}}",
        json_str(&d.task_id),
        opt(&d.choice),
        opt(&d.fallback),
        round4(d.confidence),
        d.verdict,
        scores.join(","),
        reasons.join(",")
    )
}

fn round4(x: f64) -> f64 {
    (x * 10000.0).round() / 10000.0
}

fn opt(o: &Option<String>) -> String {
    match o {
        Some(s) => json_str(s),
        None => "null".into(),
    }
}

/// JSON-safe string literal: quotes, backslash, and control characters.
fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(s: &str) -> Vec<u8> {
        route_inner(s.as_bytes())
    }

    #[test]
    fn decision_round_trip_is_valid_json() {
        let out = String::from_utf8(bytes(
            r#"{"task":{"complexity":0.2,"risk":0.1},"candidates":[{"name":"scout","tier":"fast","reliability":0.9}]}"#,
        ))
        .unwrap();
        let v = json::parse(&out).unwrap();
        assert_eq!(v.get("verdict").unwrap().as_str().unwrap(), "PASS");
        assert_eq!(v.get("choice").unwrap().as_str().unwrap(), "scout");
    }

    #[test]
    fn error_paths_are_json_too() {
        for bad in [r#"{"candidates":[]}"#, r#"not json"#] {
            let out = String::from_utf8(bytes(bad)).unwrap();
            let v = json::parse(&out).unwrap();
            assert_eq!(v.get("ok").unwrap().as_bool(), Some(false), "{out}");
        }
    }

    #[test]
    fn unicode_task_ids_survive() {
        let out = String::from_utf8(bytes(
            r#"{"task":{"id":"aurora 🦞","complexity":0.2,"risk":0.1},"candidates":[{"name":"scout 🦞","tier":"fast","reliability":0.9}]}"#,
        ))
        .unwrap();
        let v = json::parse(&out).unwrap();
        assert_eq!(v.get("task_id").unwrap().as_str().unwrap(), "aurora 🦞");
    }
}
