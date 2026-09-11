// SPDX-License-Identifier: Apache-2.0
//
// vitest's default "node" test environment has no `window`/`localStorage`.
// The dashboard only ever runs in a real browser (both exist there), so this
// is a minimal in-memory stand-in for tests only — not a jsdom replacement.

if (typeof globalThis.window === 'undefined') {
  (globalThis as unknown as { window: typeof globalThis }).window = globalThis;
}

if (typeof globalThis.localStorage === 'undefined') {
  const store = new Map<string, string>();
  const storage: Storage = {
    get length() { return store.size; },
    clear: () => store.clear(),
    getItem: (key) => (store.has(key) ? store.get(key)! : null),
    key: (index) => Array.from(store.keys())[index] ?? null,
    removeItem: (key) => { store.delete(key); },
    setItem: (key, value) => { store.set(key, value); },
  };
  (globalThis as unknown as { localStorage: Storage }).localStorage = storage;
}
