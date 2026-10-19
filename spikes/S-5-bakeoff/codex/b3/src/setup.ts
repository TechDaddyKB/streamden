import '@testing-library/jest-dom/vitest';
import { cleanup } from '@testing-library/react';
import { afterEach, vi } from 'vitest';
// Explicit browser-storage fake avoids Node 26's experimental global localStorage.
const values = new Map<string, string>();
vi.stubGlobal('localStorage', {
  getItem: (key: string) => values.get(key) ?? null,
  setItem: (key: string, value: string) => { values.set(key, value); },
  clear: () => values.clear(),
});
afterEach(() => { cleanup(); localStorage.clear(); });
