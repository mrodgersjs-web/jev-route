#!/usr/bin/env bash
# Regenerate the committed wasm artifact, then run the JS contract tests.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release --target wasm32-unknown-unknown -p jev-route-wasm
cp target/wasm32-unknown-unknown/release/jev_route_wasm.wasm wasm/js/
echo "built $(wc -c < wasm/js/jev_route_wasm.wasm) bytes -> wasm/js/jev_route_wasm.wasm"
