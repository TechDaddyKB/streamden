const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const cli = path.resolve(__dirname, '../node_modules/@commitlint/cli/cli.js');
if (!fs.existsSync(cli)) {
  console.error('Commit tooling missing: run pnpm install --frozen-lockfile --ignore-scripts in this worktree.');
  process.exit(1);
}
const result = spawnSync(process.execPath, [cli, '--edit', process.argv[2]], { stdio: 'inherit' });
process.exit(result.status ?? 1);
