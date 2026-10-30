# jev-route-wasm

The [jev-route](https://github.com/mrodgersjs-web/jev-route) deterministic
routing policy as a WebAssembly module. Task and candidates in, choice +
confidence + fallback verdict out, in the browser or any JS runtime.

Raw ABI: four exports (`alloc`, `route`, `result_len`, `dealloc`), no
wasm-bindgen, no JS glue generator, zero runtime dependencies. The policy is
the same pure Rust function the CLI runs, so a routing decision made in a
dashboard is the same decision a server would have made.

## Use

```js
import { createJevRouter } from "jev-route-wasm";

const router = await createJevRouter();
const decision = router.route({
  task: { id: "email", complexity: 0.5, risk: 0.9, outward: true },
  candidates: [
    { name: "scout", tier: "fast", reliability: 0.95 },
    { name: "workhorse", tier: "deep", reliability: 0.97, outward_ok: true },
  ],
});
// -> { verdict: "PASS", choice: "workhorse", fallback: "scout", confidence: ..., scores: {...}, reasons: [...] }
```

Bad input never traps: malformed JSON or a policy defect returns a thrown
`Error` with the Rust panic message inside, not an opaque `unreachable`.

## Build the artifact

```bash
./build.sh   # cargo build --target wasm32-unknown-unknown; copy; report size
npm test     # node:test suite against the committed .wasm (6 cases)
```

## The ABI, for auditors

`route(ptr, len)` takes ownership of the buffer `alloc(len)` returned, and
leaves the JSON result in module-owned memory: read `result_len()` bytes at
the returned pointer, copy them out, do **not** free them from JS. The
contract is documented at the top of `src/lib.rs` and enforced by the test
suite.

Part of the [mrodgersjs-web](https://github.com/mrodgersjs-web) FDE studio fleet. [rodgersintelligence.com](https://rodgersintelligence.com)
