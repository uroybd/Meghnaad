// The replica between visits: one record in IndexedDB. Whatever goes wrong here (a private window, a full disk, a
// blocked database) only means the next visit starts from the bucket, so nothing in here throws.

const DB = 'meghnaad-engine';
const STORE = 'kv';
const KEY = 'replica';

function open(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB, 1);
    req.onupgradeneeded = () => req.result.createObjectStore(STORE);
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

async function tx<T>(mode: IDBTransactionMode, use: (s: IDBObjectStore) => IDBRequest<T>): Promise<T | undefined> {
  try {
    const db = await open();
    try {
      return await new Promise<T>((resolve, reject) => {
        const t = db.transaction(STORE, mode);
        const req = use(t.objectStore(STORE));
        t.oncomplete = () => resolve(req.result);
        t.onerror = () => reject(t.error);
        t.onabort = () => reject(t.error);
      });
    } finally {
      db.close();
    }
  } catch {
    return undefined;
  }
}

export async function loadReplica(): Promise<Uint8Array | undefined> {
  const found = await tx<unknown>('readonly', (s) => s.get(KEY));
  return found instanceof Uint8Array ? found : undefined;
}

export async function saveReplica(bytes: Uint8Array): Promise<void> {
  await tx('readwrite', (s) => s.put(bytes, KEY));
}

export async function forgetReplica(): Promise<void> {
  await tx('readwrite', (s) => s.delete(KEY));
}
