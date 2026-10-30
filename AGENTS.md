# jev-route — agent guide

Rust, std-only, no dependencies. Do not add crates to fix a small problem.

## Proven commands

```bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test --release
cargo run --release -- validate --fixture fixtures/benchmark.json
```

## Constraints

- `fixtures/benchmark.json` is the policy's public contract. A scoring change
  that flips a case must either be intentional (update the case in the same
  commit and say why) or is a regression.
- Exit codes are API: 0 dispatch, 3 escalate, 4 block. Do not repurpose them.
- Keep routing a pure function: no clock, no env, no I/O inside `policy`.
- CI (`.github/workflows/ci.yml`) runs fmt --check, clippy -D warnings, tests,
  and the benchmark on every push. Never weaken it to make a push pass.
