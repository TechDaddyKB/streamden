// WCAG 2.x contrast check for every text role against the backgrounds it is used on (AA: 4.5:1).
// The focus ring must reach 3:1 against bg and surface (WCAG 1.4.11). Control borders (`control`) get 3:1; other borders are decorative and not checked.
import { tokens } from "./lib.mjs";

// --quiet prints failures only (used by `build`; keeps scripts free of shell redirection for Windows).
const quiet = process.argv.includes("--quiet");
const log = (line) => { if (!quiet) console.log(line); };

const lin = (c) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
const lum = (hex) => {
  const n = parseInt(hex.slice(1), 16);
  return 0.2126 * lin((n >> 16) / 255) + 0.7152 * lin(((n >> 8) & 255) / 255) + 0.0722 * lin((n & 255) / 255);
};
const ratio = (a, b) => {
  const [hi, lo] = [lum(a), lum(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
};

const TEXT = [
  ["text", "bg"], ["text", "surface"], ["text", "surfaceRaised"],
  ["muted", "bg"], ["muted", "surface"], ["muted", "surfaceRaised"],
  ["accent", "bg"], ["accent", "surface"],
  ["success", "bg"], ["success", "surface"],
  ["warning", "bg"], ["warning", "surface"],
  ["danger", "bg"], ["danger", "surface"],
  ["accent", "surfaceRaised"], ["success", "surfaceRaised"], ["warning", "surfaceRaised"], ["danger", "surfaceRaised"],
  ["onAccent", "accent"],
];
const NON_TEXT = [["focus", "bg"], ["focus", "surface"], ["focus", "surfaceRaised"],
  ["control", "bg"], ["control", "surface"], ["control", "surfaceRaised"]];

let failed = false;
for (const [name, t] of Object.entries(tokens.color.themes)) {
  log(`\n${name} theme`);
  for (const [min, pairs] of [[4.5, TEXT], [3, NON_TEXT]]) {
    for (const [fg, bg] of pairs) {
      const r = ratio(t[fg], t[bg]);
      const ok = r >= min;
      failed ||= !ok;
      if (!ok || !quiet) console.log(`  ${ok ? "AA pass" : "FAIL   "} ${r.toFixed(2).padStart(5)}:1 (min ${min})  ${fg} on ${bg}`);
    }
  }
}
if (failed) process.exit(1);
log("\nAll text roles pass WCAG AA.");
