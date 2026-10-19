import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { buildCss, isCssCurrent } from "./lib.mjs";

const committed = readFileSync(new URL("../src/tokens.css", import.meta.url), "utf8");

test("committed tokens.css matches the generator", () => {
  assert.ok(isCssCurrent(committed, buildCss()));
});

test("a CRLF copy (Windows checkout) compares equal", () => {
  const crlf = committed.replace(/\r?\n/g, "\r\n");
  assert.ok(crlf.includes("\r\n"));
  assert.ok(isCssCurrent(crlf, buildCss()));
});

test("real content drift is still detected, with either line ending", () => {
  const drifted = committed.replace("--sd-", "--sx-");
  assert.ok(!isCssCurrent(drifted, buildCss()));
  assert.ok(!isCssCurrent(drifted.replace(/\n/g, "\r\n"), buildCss()));
});
