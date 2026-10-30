import test from "node:test";
import assert from "node:assert/strict";
import { createJevRouter } from "../js/index.js";

const router = await createJevRouter();

test("right-sizing beats muscle", () => {
  const d = router.route({
    task: { id: "fmt", complexity: 0.2, risk: 0.1 },
    candidates: [
      { name: "workhorse", tier: "deep", reliability: 0.97, outward_ok: true },
      { name: "scout", tier: "fast", reliability: 0.9 },
    ],
  });
  assert.equal(d.verdict, "PASS");
  assert.equal(d.choice, "scout");
});

test("outward work gates on approval", () => {
  const d = router.route({
    task: { complexity: 0.5, risk: 0.9, outward: true },
    candidates: [
      { name: "scout", tier: "fast", reliability: 0.95 },
      { name: "workhorse", tier: "deep", reliability: 0.97, outward_ok: true },
    ],
  });
  assert.equal(d.choice, "workhorse");
});

test("blind tier is excluded from image tasks", () => {
  const d = router.route({
    task: { complexity: 0.6, risk: 0.4, needs_images: true },
    candidates: [{ name: "scout", tier: "fast", reliability: 1.0 }],
  });
  assert.equal(d.verdict, "BLOCK");
});

test("deterministic: same input, same scores", () => {
  const input = {
    task: { complexity: 0.45, risk: 0.5 },
    candidates: [
      { name: "zeta", tier: "standard", reliability: 0.9 },
      { name: "alpha", tier: "standard", reliability: 0.9 },
    ],
  };
  assert.deepEqual(router.route(input), router.route(input));
});

test("malformed input throws with the error message", () => {
  assert.throws(() => router.route({ candidates: [] }), /missing "task"/);
});

test("unicode survives the round trip", () => {
  const d = router.route({
    task: { id: "aurora 🦞", complexity: 0.2, risk: 0.1 },
    candidates: [{ name: "scout 🦞", tier: "fast", reliability: 0.9 }],
  });
  assert.equal(d.task_id, "aurora 🦞");
  assert.equal(d.choice, "scout 🦞");
});
