const { execFileSync } = require('node:child_process');
const { readFileSync, existsSync } = require('node:fs');

function currentBranch() {
  const git = args => execFileSync('git', args, { encoding: 'utf8' }).trim();
  const branch = git(['branch', '--show-current']);
  if (branch) return branch;
  // Rebase hooks run on detached HEAD; head-name retains the original branch.
  for (const state of ['rebase-merge', 'rebase-apply']) {
    const path = git(['rev-parse', '--git-path', `${state}/head-name`]);
    if (existsSync(path)) {
      const ref = readFileSync(path, 'utf8').trim();
      return ref.startsWith('refs/heads/') ? ref.slice('refs/heads/'.length) : '';
    }
  }
  return '';
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
