import { writeFileSync } from 'node:fs';
const subjects = {
  Core: ['Variable', 'Counter', 'Timer', 'Action', 'Queue'],
  Twitch: ['Chat message', 'Channel reward', 'Stream title', 'Poll', 'Prediction'],
  OBS: ['Scene', 'Source visibility', 'Audio filter', 'Recording', 'Replay buffer'],
  Audio: ['Sound', 'Voice', 'Playlist', 'Volume', 'Microphone'],
  Integrations: ['Webhook', 'Overlay', 'Discord message', 'VTuber expression', 'YouTube chat'],
};
const verbs = ['Set', 'Get', 'Toggle', 'Reset', 'Enable', 'Disable', 'Wait for', 'Log'];
const data = Object.entries(subjects).flatMap(([category, names]) => names.flatMap((subject, s) => verbs.map((verb, v) => ({ id: `${category.toLowerCase()}-${s}-${v}`, category, name: `${verb} ${subject.toLowerCase()}`, description: `${verb} the ${subject.toLowerCase()} in ${category}. Generated demonstration action; no service connection.` }))));
writeFileSync(new URL('../src/actions.json', import.meta.url), JSON.stringify(data, null, 0) + '\n');
console.log(`Generated ${data.length} actions`);
