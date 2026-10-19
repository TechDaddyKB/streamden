import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const manifest = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
const outputDir = join(root, "generated");
mkdirSync(outputDir, { recursive: true });

const stamp = {
  generator: "grok-b4-placeholder",
  name: manifest.name,
  version: manifest.version,
};

writeFileSync(join(outputDir, "stamp.json"), `${JSON.stringify(stamp, null, 2)}\n`);
