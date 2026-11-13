// The replica between visits: one record in IndexedDB. A read that fails (a private window, a blocked database) only
// means this visit starts from the bucket, so reading never throws. A write that fails does: the page tells people
// that what they change is kept in this browser, and it must know when that stopped being true.

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

async function tx<T>(mode: IDBTransactionMode, use: (s: IDBObjectStore) => IDBRequest<T>): Promise<T> {
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
  // Unreadable is the same as no copy at all: the bucket has everything that was pushed.
  const found = await tx<unknown>('readonly', (s) => s.get(KEY)).catch(() => undefined);
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
