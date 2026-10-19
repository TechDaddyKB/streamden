import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
const { version } = JSON.parse(readFileSync('package.json', 'utf8'));
mkdirSync('generated', { recursive: true });
writeFileSync('generated/version.json', `${JSON.stringify({ version }, null, 2)}\n`);
