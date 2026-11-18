// For tests only: an IndexedDB in memory, enough for persist.ts and vault.ts.

/** An IndexedDB in memory: one database, one store, the few calls persist.ts makes. */
export function memoryDb() {
  const data = new Map<unknown, unknown>();
  const request = <T>(value: () => T) => {
    const req: { result?: T; onsuccess?: () => void } = {};
    queueMicrotask(() => {
      req.result = value();
      req.onsuccess?.();
    });
    return req;
  };
  const db = {
    transaction() {
      const t: { oncomplete?: () => void; objectStore: () => unknown } = {
        objectStore: () => ({
          get: (k: unknown) => request(() => data.get(k)),
          put: (v: unknown, k: unknown) => request(() => data.set(k, v)),
          delete: (k: unknown) =>
            request(() => {
              // A key range (the stand-in for IDBKeyRange.bound below), or one key.
              if (k && typeof k === 'object' && 'lo' in k && 'hi' in k) {
                const { lo, hi } = k as { lo: string; hi: string };
                for (const key of [...data.keys()])
                  if (typeof key === 'string' && key >= lo && key <= hi) data.delete(key);
              } else data.delete(k);
            }),
        }),
      };
      setTimeout(() => t.oncomplete?.(), 0);
      return t;
    },
    close() {},
  };
  return {
    data,
    indexedDB: {
      deleteDatabase() {
        const req: { onsuccess?: () => void } = {};
        queueMicrotask(() => {
          data.clear();
          req.onsuccess?.();
        });
        return req;
      },
      open() {
        const req: { result: typeof db; onsuccess?: () => void } = { result: db };
        queueMicrotask(() => req.onsuccess?.());
        return req;
      },
    },
  };
}
