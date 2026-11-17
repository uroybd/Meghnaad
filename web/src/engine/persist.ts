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

export async function tx<T>(mode: IDBTransactionMode, use: (s: IDBObjectStore) => IDBRequest<T>): Promise<T> {
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

// The static flavour keeps the copy sealed (AES-GCM, with a key made from the key of the tasks), since nothing else
// stands between this browser's storage and the tasks: a sealed record starts with SEALED, then the IV.
const SEALED = new TextEncoder().encode('MGS1');
const IV = 12;
let sealKey: CryptoKey | undefined;

/** Seal the copy from now on with a key made from `key` (the key of the tasks, 32 bytes); it can't be read back out. */
export async function sealWith(key: Uint8Array): Promise<void> {
  const base = await crypto.subtle.importKey('raw', key as Uint8Array<ArrayBuffer>, 'HKDF', false, ['deriveKey']);
  sealKey = await crypto.subtle.deriveKey(
    { name: 'HKDF', hash: 'SHA-256', salt: new Uint8Array(32), info: new TextEncoder().encode('meghnaad replica') },
    base,
    { name: 'AES-GCM', length: 256 },
    false,
    ['encrypt', 'decrypt'],
  );
}

function isSealed(bytes: Uint8Array): boolean {
  return bytes.length > SEALED.length + IV && SEALED.every((b, i) => bytes[i] === b);
}

async function sealed(bytes: Uint8Array, key: CryptoKey): Promise<Uint8Array> {
  const iv = crypto.getRandomValues(new Uint8Array(IV));
  const out = new Uint8Array(
    await crypto.subtle.encrypt({ name: 'AES-GCM', iv }, key, bytes as Uint8Array<ArrayBuffer>),
  );
  const all = new Uint8Array(SEALED.length + IV + out.length);
  all.set(SEALED);
  all.set(iv, SEALED.length);
  all.set(out, SEALED.length + IV);
  return all;
}

async function opened(bytes: Uint8Array, key: CryptoKey): Promise<Uint8Array> {
  const iv = bytes.subarray(SEALED.length, SEALED.length + IV);
  const body = bytes.subarray(SEALED.length + IV);
  return new Uint8Array(
    await crypto.subtle.decrypt(
      { name: 'AES-GCM', iv: iv as Uint8Array<ArrayBuffer> },
      key,
      body as Uint8Array<ArrayBuffer>,
    ),
  );
}

export async function loadReplica(): Promise<Uint8Array | undefined> {
  // Unreadable is the same as no copy at all: the bucket has everything that was pushed.
  const found = await tx<unknown>('readonly', (s) => s.get(KEY)).catch(() => undefined);
  if (!(found instanceof Uint8Array)) return undefined;
  try {
    // Sealing on, a copy that isn't sealed is not this device's to trust; sealing off, a sealed one can't be opened.
    if (sealKey ? !isSealed(found) : isSealed(found)) return undefined;
    return await unpack(sealKey ? await opened(found, sealKey) : found);
  } catch {
    // Damaged: the same as no copy at all, and the next visit starts from the bucket.
    return undefined;
  }
}

export async function saveReplica(bytes: Uint8Array): Promise<void> {
  const packed = await pack(bytes);
  const kept = sealKey ? await sealed(packed, sealKey) : packed;
  await tx('readwrite', (s) => s.put(kept, KEY));
}

export async function forgetReplica(): Promise<void> {
  await tx('readwrite', (s) => s.delete(KEY));
}

// The static flavour keeps the settings it last read (the answers of `KEPT_ROUTES`), sealed as the copy of the tasks
// is, so unlocking starts from them and reads the branch's after. Kept only while sealing is on.
const KEPT = 'kept:';

/** What was kept of a route, or nothing: never kept, sealing off, or unreadable. */
export async function loadKept(path: string): Promise<string | undefined> {
  if (!sealKey) return undefined;
  const found = await tx<unknown>('readonly', (s) => s.get(KEPT + path)).catch(() => undefined);
  if (!(found instanceof Uint8Array) || !isSealed(found)) return undefined;
  try {
    return new TextDecoder().decode(await opened(found, sealKey));
  } catch {
    return undefined;
  }
}

export async function keepAnswer(path: string, text: string): Promise<void> {
  if (!sealKey) return;
  const kept = await sealed(new TextEncoder().encode(text), sealKey);
  await tx('readwrite', (s) => s.put(kept, KEPT + path));
}

/** Forget every kept route: a setting was written, and the next read is the branch's. */
export async function forgetKept(): Promise<void> {
  await tx('readwrite', (s) => s.delete(IDBKeyRange.bound(KEPT, `${KEPT}\uffff`)));
}
