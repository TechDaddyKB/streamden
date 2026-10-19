import assert from "node:assert/strict";
import test from "node:test";
import { add } from "../src/index.js";

test("adds finite numbers", () => {
  assert.equal(add(2, 3), 5);
});

test("rejects non-finite input", () => {
  assert.throws(() => add(Number.NaN, 1), TypeError);
});
