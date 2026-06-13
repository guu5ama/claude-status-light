import '@testing-library/jest-dom/vitest';

// jsdom in this environment does not expose window.localStorage; give tests
// a minimal in-memory implementation so components touching it can mount.
function ensureLocalStorage() {
  try {
    if (window.localStorage) {
      return;
    }
  } catch {
    // Opaque-origin jsdom throws on access; replace it below.
  }

  const store = new Map<string, string>();
  Object.defineProperty(window, 'localStorage', {
    configurable: true,
    value: {
      getItem: (key: string) => store.get(String(key)) ?? null,
      setItem: (key: string, value: string) => {
        store.set(String(key), String(value));
      },
      removeItem: (key: string) => {
        store.delete(String(key));
      },
      clear: () => {
        store.clear();
      },
      key: (index: number) => Array.from(store.keys())[index] ?? null,
      get length() {
        return store.size;
      }
    }
  });
}

if (typeof window !== 'undefined') {
  ensureLocalStorage();
}
