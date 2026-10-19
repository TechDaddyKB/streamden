// Writes src/tokens.css from src/tokens.json. `--check` fails if the committed file is stale.
import { readFileSync, writeFileSync } from "node:fs";
import { buildCss } from "./lib.mjs";

const target = new URL("../src/tokens.css", import.meta.url);
const css = buildCss();
if (process.argv.includes("--check")) {
  let current = "";
  try { current = readFileSync(target, "utf8"); } catch {}
  if (current !== css) {
    console.error("src/tokens.css is stale: run `pnpm --filter @streamden/kit tokens:css`");
    process.exit(1);
  }
  console.log("tokens.css is up to date");
} else {
  writeFileSync(target, css);
  console.log("wrote src/tokens.css");
}
