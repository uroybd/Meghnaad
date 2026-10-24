// A zip file of text files, stored uncompressed: the taskrc folder is small, and this is all "Download all" needs, so no
// library is carried for it. (The Android app has the same writer in `RcZip.kt`.)

const TABLE = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();

export function crc32(bytes: Uint8Array): number {
  let c = 0xffffffff;
  for (const b of bytes) c = TABLE[(c ^ b) & 0xff]! ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

/** The folder's files as a zip, each at its path (`taskrc`, `holidays/en-US.rc`), its names read as UTF-8. */
export function zipFiles(files: { path: string; text: string }[], when: Date = new Date()): Uint8Array<ArrayBuffer> {
  const enc = new TextEncoder();
  // DOS time and date; a zip cannot write a year before 1980.
  const year = Math.max(1980, when.getFullYear());
  const time = (when.getHours() << 11) | (when.getMinutes() << 5) | (when.getSeconds() >> 1);
  const date = ((year - 1980) << 9) | ((when.getMonth() + 1) << 5) | when.getDate();

  const entries = files.map((f) => ({ name: enc.encode(f.path), data: enc.encode(f.text) }));
  const size = entries.reduce((n, e) => n + 30 + 46 + 2 * e.name.length + e.data.length, 22);
  const out = new Uint8Array(size);
  const view = new DataView(out.buffer);
  let at = 0;
  const u16 = (v: number) => (view.setUint16(at, v, true), (at += 2));
  const u32 = (v: number) => (view.setUint32(at, v, true), (at += 4));
  const put = (b: Uint8Array) => (out.set(b, at), (at += b.length));

  const offsets: number[] = [];
  for (const e of entries) {
    offsets.push(at);
    u32(0x04034b50);
    u16(20); // version needed
    u16(0x0800); // names are UTF-8
    u16(0); // stored
    u16(time);
    u16(date);
    u32(crc32(e.data));
    u32(e.data.length);
    u32(e.data.length);
    u16(e.name.length);
    u16(0);
    put(e.name);
    put(e.data);
  }
  const directory = at;
  entries.forEach((e, i) => {
    u32(0x02014b50);
    u16(20); // made by
    u16(20); // version needed
    u16(0x0800);
    u16(0);
    u16(time);
    u16(date);
    u32(crc32(e.data));
    u32(e.data.length);
    u32(e.data.length);
    u16(e.name.length);
    u16(0); // extra
    u16(0); // comment
    u16(0); // disk
    u16(0); // internal attributes
    u32(0); // external attributes
    u32(offsets[i]!);
    put(e.name);
  });
  const length = at - directory;
  u32(0x06054b50);
  u16(0);
  u16(0);
  u16(entries.length);
  u16(entries.length);
  u32(length);
  u32(directory);
  u16(0);
  return out;
}
