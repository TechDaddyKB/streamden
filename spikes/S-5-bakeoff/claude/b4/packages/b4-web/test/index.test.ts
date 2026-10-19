import assert from "node:assert/strict";
import { test } from "node:test";
import { parseEventKind } from "../src/index.ts";

test("parses known kinds and rejects unknown ones", () => {
  assert.equal(parseEventKind("raid"), "raid");
  assert.equal(parseEventKind("Raid"), undefined);
});
