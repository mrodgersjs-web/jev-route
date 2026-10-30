# jev-route

A deterministic routing-policy CLI in dependency-free Rust: a task and a
candidate list go in, a choice, a confidence and a fallback verdict come out.
The policy lives in `src/policy.rs` (pure functions, no I/O); `src/json.rs` is
a std-only reader for the input files; `src/bench.rs` runs the checked-in
benchmark; `src/main.rs` wires argv.

## Layout

- `src/policy.rs` - tiers, candidates, tasks, scoring, the route entry point.
- `src/json.rs` / `src/bench.rs` - input parsing and the benchmark harness.
- `fixtures/benchmark.json` - the policy contract: cases with expected verdicts
  and choices. Treat a flipped case as a regression unless the commit says why.
- `tests/policy_tests.rs` - integration tests, including a green-fixture gate.
- `.github/workflows/ci.yml` - fmt, clippy -D warnings, tests, benchmark.

## Setup and test (exact commands)

```bash
cargo test --release
cargo run --release -- validate --fixture fixtures/benchmark.json
```

## Rules

- std-only. No new dependencies without a spec'd reason.
- Exit codes are API: 0 dispatch, 3 escalate, 4 block.
- Routing must stay deterministic: no clock, no env, no network in the policy.
- The validate command below is the repo smoke: keep it runnable and keep CI green.
