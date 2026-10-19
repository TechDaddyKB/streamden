const { execFileSync } = require('node:child_process');

function currentBranch() {
  return execFileSync('git', ['branch', '--show-current'], { encoding: 'utf8' }).trim();
}

function validBranch(branch) {
  return /^(?:main|owner\/.+|agent\/(?:claude|codex|grok|other)\/DEN-\d+-[a-z0-9-]+)$/.test(branch);
}

module.exports = { currentBranch, validBranch };

if (require.main === module) {
  const branch = process.argv[2] ?? currentBranch();
  if (!validBranch(branch)) {
    console.error(`Invalid branch: ${branch || '(detached HEAD)'}. Use agent/<vendor>/DEN-<n>-<slug>, owner/<slug>, or main.`);
    process.exitCode = 1;
  } else {
    console.log(`Branch OK: ${branch}`);
  }
}
