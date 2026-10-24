import { describe, expect, it } from 'vitest';
import { crc32, zipFiles } from './zip';

/** A reader of what the writer makes, going by the central directory as unzip does. */
function read(zip: Uint8Array) {
  const v = new DataView(zip.buffer, zip.byteOffset, zip.byteLength);
  const end = zip.length - 22;
  expect(v.getUint32(end, true)).toBe(0x06054b50);
  const count = v.getUint16(end + 10, true);
  let at = v.getUint32(end + 16, true);
  const dec = new TextDecoder();
  const out: { name: string; text: string; crc: number }[] = [];
  for (let i = 0; i < count; i++) {
    expect(v.getUint32(at, true)).toBe(0x02014b50);
    const size = v.getUint32(at + 24, true);
    const nameLen = v.getUint16(at + 28, true);
    const local = v.getUint32(at + 42, true);
    const name = dec.decode(zip.subarray(at + 46, at + 46 + nameLen));
    expect(v.getUint32(local, true)).toBe(0x04034b50);
    const start = local + 30 + v.getUint16(local + 26, true) + v.getUint16(local + 28, true);
    out.push({ name, text: dec.decode(zip.subarray(start, start + size)), crc: v.getUint32(at + 16, true) });
    at += 46 + nameLen;
  }
  return out;
}

describe('zipFiles', () => {
  it('has the standard checksum', () => {
    expect(crc32(new TextEncoder().encode('123456789'))).toBe(0xcbf43926);
    expect(crc32(new Uint8Array())).toBe(0);
  });

  it('keeps every file, its folder and its text', () => {
    const files = [
      { path: 'taskrc', text: 'bulk=3\ninclude holidays/en-US.rc\n' },
      { path: 'holidays/en-US.rc', text: 'holiday.en-US1.name=Fête\n' },
      { path: 'empty.rc', text: '' },
    ];
    const got = read(zipFiles(files, new Date(2026, 9, 24, 12, 30, 10)));
    expect(got.map((g) => [g.name, g.text])).toEqual(files.map((f) => [f.path, f.text]));
    expect(got.map((g) => g.crc)).toEqual(files.map((f) => crc32(new TextEncoder().encode(f.text))));
  });

  it('is a valid, empty archive when there are no files', () => {
    const z = zipFiles([]);
    expect(z.length).toBe(22);
    expect(read(z)).toEqual([]);
  });
});
