// Vitest global setup: guarantee a working Web Storage API under jsdom.
//
// Node 25 ships a built-in `globalThis.localStorage` that is only functional
// with `--localstorage-file`; without that flag the object exists but its
// get/set/remove methods are absent, and jsdom does not replace it. Tests that
// touch storage (proposal dismissal, auth, wallet, active-portfolio) then throw
// "window.localStorage.setItem is not a function". Installing a real in-memory
// Storage keeps the suite behaving identically across Node 20–25.

function createMemoryStorage(): Storage {
  const store = new Map<string, string>();
  return {
    get length() {
      return store.size;
    },
    clear() {
      store.clear();
    },
    getItem(key: string) {
      return store.has(key) ? (store.get(key) ?? null) : null;
    },
    key(index: number) {
      return Array.from(store.keys())[index] ?? null;
    },
    removeItem(key: string) {
      store.delete(key);
    },
    setItem(key: string, value: string) {
      store.set(key, String(value));
    },
  } as Storage;
}

function ensureStorage(name: "localStorage" | "sessionStorage") {
  const existing = (globalThis as { [k: string]: unknown })[name] as
    | Storage
    | undefined;
  const usable =
    !!existing &&
    typeof existing.getItem === "function" &&
    typeof existing.setItem === "function" &&
    typeof existing.removeItem === "function";
  if (usable) return;

  const storage = createMemoryStorage();
  Object.defineProperty(globalThis, name, {
    value: storage,
    configurable: true,
    writable: true,
  });
  if (typeof window !== "undefined") {
    Object.defineProperty(window, name, {
      value: storage,
      configurable: true,
      writable: true,
    });
  }
}

ensureStorage("localStorage");
ensureStorage("sessionStorage");
