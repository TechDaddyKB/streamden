import fixture from './actions.json';
export const actions = fixture;
export type Action = (typeof actions)[number];
export const categories = [...new Set(actions.map(a => a.category))];
// Ordered subsequence matching: contiguous text ranks ahead of fuzzy abbreviations.
export function score(text: string, query: string): number {
  const haystack = text.toLowerCase(), needle = query.toLowerCase().replace(/\s/g, '');
  let at = -1, cost = 0;
  for (const char of needle) {
    const next = haystack.indexOf(char, at + 1);
    if (next < 0) return Infinity;
    cost += next - at - 1; at = next;
  }
  return cost;
}
export function load(key: string): string[] {
  try {
    const value: unknown = JSON.parse(localStorage.getItem(key) ?? '[]');
    return Array.isArray(value) ? [...new Set(value.filter((id): id is string => typeof id === 'string' && actions.some(a => a.id === id)))] : [];
  } catch { return []; }
}
export function save(key: string, value: string[]) {
  try { localStorage.setItem(key, JSON.stringify(value)); } catch { /* In-memory operation remains available. */ }
}
