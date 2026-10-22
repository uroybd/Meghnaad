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

/**
 * What the engine saves is JSON, and mostly the changes made on this device (kept as Taskwarrior keeps them, for the
 * history of a task): about 17 times smaller as gzip, which the browser does itself. A record is written compressed when
 * it can be; one written before this (JSON text, which starts with `{`) is still read.
 */
const GZIP_MAGIC = [0x1f, 0x8b];

function isGzip(bytes: Uint8Array): boolean {
  return bytes.length > GZIP_MAGIC.length && GZIP_MAGIC.every((b, i) => bytes[i] === b);
}

async function through(bytes: Uint8Array, stream: CompressionStream | DecompressionStream): Promise<Uint8Array> {
  const piped = new Blob([bytes as Uint8Array<ArrayBuffer>]).stream().pipeThrough(stream);
  return new Uint8Array(await new Response(piped).arrayBuffer());
}

/** The bytes to keep: compressed, or as they are where the browser cannot compress. */
export async function pack(bytes: Uint8Array): Promise<Uint8Array> {
  if (typeof CompressionStream === 'undefined') return bytes;
  try {
    return await through(bytes, new CompressionStream('gzip'));
  } catch {
    return bytes;
  }
}

/** The engine's bytes back from what [`pack`] (or an older version of this file) kept. */
export async function unpack(kept: Uint8Array): Promise<Uint8Array> {
  return isGzip(kept) ? through(kept, new DecompressionStream('gzip')) : kept;
}

export async function loadReplica(): Promise<Uint8Array | undefined> {
  const found = await tx<unknown>('readonly', (s) => s.get(KEY));
  if (!(found instanceof Uint8Array)) return undefined;
  try {
    return await unpack(found);
  } catch {
    // Damaged: the same as no copy at all, and the next visit starts from the bucket.
    return undefined;
  }
}

export async function saveReplica(bytes: Uint8Array): Promise<void> {
  const kept = await pack(bytes);
  await tx('readwrite', (s) => s.put(kept, KEY));
}

export async function forgetReplica(): Promise<void> {
  await tx('readwrite', (s) => s.delete(KEY));
}
