// A message sealed with a password in OpenPGP's format (RFC 4880: what `gpg --symmetric` writes, and what
// `gpg --decrypt` opens): a symmetric-key session key packet (AES-256, iterated and salted S2K with SHA-256) and an
// integrity-protected data packet (AES-256 in CFB mode, with its SHA-1 modification detection code). The static web app
// keeps the token for the git repository this way, sealed with the encryption secret, so a copy can be opened outside
// the app too. Only WebCrypto is used: nothing here is in the engine's WebAssembly.

const AES256 = 9;
const SHA256 = 8;
const S2K_ITERATED = 3;
/** The most work the format allows: 65,011,712 bytes hashed to make the key. */
const S2K_COUNT_BYTE = 0xff;
const BLOCK = 16;

const TAG_SKESK = 3;
const TAG_LITERAL = 11;
const TAG_SEIPD = 18;
const TAG_MDC = 19;

/** The password was wrong, or the message is not one this reads (or was changed). */
export class WrongPassword extends Error {
  constructor() {
    super('That is not the encryption secret for this device.');
  }
}

const subtle = (): SubtleCrypto => globalThis.crypto.subtle;
const buf = (b: Uint8Array): Uint8Array<ArrayBuffer> => b as Uint8Array<ArrayBuffer>;

function concat(...parts: Uint8Array[]): Uint8Array {
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let at = 0;
  for (const p of parts) {
    out.set(p, at);
    at += p.length;
  }
  return out;
}

/** A packet in the new format: its tag and a length of one, two or five bytes. */
function packet(tag: number, body: Uint8Array): Uint8Array {
  const n = body.length;
  let len: number[];
  if (n < 192) len = [n];
  else if (n < 8384) len = [((n - 192) >> 8) + 192, (n - 192) & 0xff];
  else len = [0xff, (n >>> 24) & 0xff, (n >>> 16) & 0xff, (n >>> 8) & 0xff, n & 0xff];
  return concat(new Uint8Array([0xc0 | tag, ...len]), body);
}

interface Packet {
  tag: number;
  body: Uint8Array;
}

/** The packets of a message, in the new format or the old (which gpg may write). */
function packets(bytes: Uint8Array): Packet[] {
  const out: Packet[] = [];
  let at = 0;
  while (at < bytes.length) {
    const head = bytes[at++];
    if (!(head & 0x80)) throw new WrongPassword();
    let tag: number;
    let len: number;
    if (head & 0x40) {
      tag = head & 0x3f;
      const first = bytes[at++];
      if (first < 192) len = first;
      else if (first < 224) len = ((first - 192) << 8) + bytes[at++] + 192;
      else if (first === 255) {
        len = ((bytes[at] << 24) | (bytes[at + 1] << 16) | (bytes[at + 2] << 8) | bytes[at + 3]) >>> 0;
        at += 4;
      } else throw new WrongPassword(); // partial lengths: not written by this, nor by gpg for a small message
    } else {
      tag = (head >> 2) & 0x0f;
      const kind = head & 3;
      if (kind === 3) len = bytes.length - at;
      else {
        const size = [1, 2, 4][kind];
        len = 0;
        for (let i = 0; i < size; i++) len = len * 256 + bytes[at + i];
        at += size;
      }
    }
    if (at + len > bytes.length) throw new WrongPassword();
    out.push({ tag, body: bytes.subarray(at, at + len) });
    at += len;
  }
  return out;
}

function s2kCount(c: number): number {
  return (16 + (c & 15)) << ((c >> 4) + 6);
}

/** The iterated and salted S2K with SHA-256: the salt and the password, repeated to `count` bytes, hashed once. */
async function s2k(password: string, salt: Uint8Array, countByte: number): Promise<CryptoKey> {
  const unit = concat(salt, new TextEncoder().encode(password));
  const count = Math.max(s2kCount(countByte), unit.length);
  const data = new Uint8Array(count);
  for (let at = 0; at < count; at += unit.length) data.set(unit.subarray(0, Math.min(unit.length, count - at)), at);
  const key = await subtle().digest('SHA-256', data);
  return subtle().importKey('raw', key, 'AES-CBC', false, ['encrypt']);
}

/** One AES block, encrypted: CBC over one block with a zero IV is that, and WebCrypto has no plain AES. */
async function aesBlock(key: CryptoKey, block: Uint8Array): Promise<Uint8Array> {
  const out = await subtle().encrypt({ name: 'AES-CBC', iv: new Uint8Array(BLOCK) }, key, buf(block));
  return new Uint8Array(out, 0, BLOCK);
}

/** CFB with a zero IV, as an integrity-protected data packet uses it. */
async function cfb(key: CryptoKey, data: Uint8Array, decrypt: boolean): Promise<Uint8Array> {
  const out = new Uint8Array(data.length);
  if (decrypt) {
    // Each block's keystream is the cipher of the ciphertext before it, all known at once.
    const ivs = Array.from({ length: Math.ceil(data.length / BLOCK) }, (_, i) =>
      i === 0 ? new Uint8Array(BLOCK) : data.subarray((i - 1) * BLOCK, i * BLOCK),
    );
    const streams = await Promise.all(ivs.map((iv) => aesBlock(key, iv)));
    for (let i = 0; i < data.length; i++) out[i] = data[i] ^ streams[Math.floor(i / BLOCK)][i % BLOCK];
    return out;
  }
  let prev: Uint8Array = new Uint8Array(BLOCK);
  for (let at = 0; at < data.length; at += BLOCK) {
    const stream = await aesBlock(key, prev);
    const end = Math.min(at + BLOCK, data.length);
    for (let i = at; i < end; i++) out[i] = data[i] ^ stream[i - at];
    prev = out.subarray(at, at + BLOCK);
  }
  return out;
}

async function sha1(data: Uint8Array): Promise<Uint8Array> {
  return new Uint8Array(await subtle().digest('SHA-1', buf(data)));
}

/** `data` sealed with `password`: a binary OpenPGP message. */
export async function seal(data: Uint8Array, password: string): Promise<Uint8Array> {
  const salt = crypto.getRandomValues(new Uint8Array(8));
  const key = await s2k(password, salt, S2K_COUNT_BYTE);
  const skesk = packet(TAG_SKESK, new Uint8Array([4, AES256, S2K_ITERATED, SHA256, ...salt, S2K_COUNT_BYTE]));
  // A binary literal: no file name, no date.
  const literal = packet(TAG_LITERAL, concat(new Uint8Array([0x62, 0, 0, 0, 0, 0]), data));
  const random = crypto.getRandomValues(new Uint8Array(BLOCK));
  const prefix = concat(random, random.subarray(BLOCK - 2));
  const mdcHead = new Uint8Array([0xc0 | TAG_MDC, 20]);
  const hash = await sha1(concat(prefix, literal, mdcHead));
  const encrypted = await cfb(key, concat(prefix, literal, mdcHead, hash), false);
  return concat(skesk, packet(TAG_SEIPD, concat(new Uint8Array([1]), encrypted)));
}

/** What [`seal`] sealed, given the same password; `WrongPassword` otherwise. */
export async function open(message: Uint8Array, password: string): Promise<Uint8Array> {
  const found = packets(message);
  const skesk = found.find((p) => p.tag === TAG_SKESK)?.body;
  const seipd = found.find((p) => p.tag === TAG_SEIPD)?.body;
  // Only what `seal` writes: version 4, AES-256, iterated and salted SHA-256, no separate session key.
  if (!skesk || !seipd || skesk.length !== 13 || seipd[0] !== 1) throw new WrongPassword();
  if (skesk[0] !== 4 || skesk[1] !== AES256 || skesk[2] !== S2K_ITERATED || skesk[3] !== SHA256) {
    throw new WrongPassword();
  }
  const key = await s2k(password, skesk.subarray(4, 12), skesk[12]);
  const plain = await cfb(key, seipd.subarray(1), true);
  if (plain.length < BLOCK + 2 + 22) throw new WrongPassword();
  // The quick check: the prefix's last two bytes again. A wrong password fails here.
  if (plain[BLOCK] !== plain[BLOCK - 2] || plain[BLOCK + 1] !== plain[BLOCK - 1]) throw new WrongPassword();
  const body = plain.subarray(0, plain.length - 20);
  const mdc = plain.subarray(plain.length - 20);
  const expected = await sha1(body);
  if (body[body.length - 2] !== (0xc0 | TAG_MDC) || body[body.length - 1] !== 20) throw new WrongPassword();
  if (!expected.every((b, i) => b === mdc[i])) throw new WrongPassword();
  const inner = packets(body.subarray(BLOCK + 2, body.length - 2));
  const literal = inner.find((p) => p.tag === TAG_LITERAL)?.body;
  if (!literal) throw new WrongPassword();
  const nameLength = literal[1];
  return literal.slice(2 + nameLength + 4);
}
