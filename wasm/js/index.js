// jev-route-wasm: the deterministic routing policy as a WebAssembly module.
// Raw ABI (alloc/route/result_len/dealloc) - no bindgen, no runtime deps.
//
//   import { createJevRouter } from "jev-route-wasm";
//   const router = await createJevRouter();          // loads ./jev_route_wasm.wasm
//   const decision = router.route({ task, candidates });

import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

export async function createJevRouter(wasmBytes) {
  if (!wasmBytes) {
    const here = dirname(fileURLToPath(import.meta.url));
    wasmBytes = await readFile(join(here, "jev_route_wasm.wasm"));
  }
  const { instance } = await WebAssembly.instantiate(wasmBytes, {});
  const mem = new Uint8Array(instance.exports.memory.buffer);
  const encoder = new TextEncoder();
  const decoder = new TextDecoder();

  return {
    route(input) {
      const bytes = encoder.encode(JSON.stringify(input));
      const inPtr = instance.exports.alloc(bytes.length);
      new Uint8Array(instance.exports.memory.buffer).set(bytes, inPtr);
      const outPtr = instance.exports.route(inPtr, bytes.length);
      const outLen = instance.exports.result_len();
      if (outPtr === 0 || outLen === 0) {
        throw new Error("jev-route-wasm: empty result");
      }
      // The result buffer is module-owned (valid until the next call):
      // copy it out, never free it from JS. The input buffer is consumed
      // by route() itself.
      const view = new Uint8Array(instance.exports.memory.buffer, outPtr, outLen);
      const result = JSON.parse(decoder.decode(view));
      if (result.ok === false) throw new Error(`jev-route-wasm: ${result.error}`);
      return result;
    },
  };
}

export default createJevRouter;
