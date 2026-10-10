// Takes sections the runtime never reads out of a WebAssembly binary. The "name" section holds
// function names for stack traces and profilers; in the Worker's module it is over a fifth of the
// file and about 70 KB of what gets uploaded.

/** Read an unsigned LEB128 number at `i`: [value, index after it]. */
function leb(bytes, i) {
  let value = 0;
  let shift = 0;
  let b;
  do {
    b = bytes[i++];
    value += (b & 0x7f) * 2 ** shift;
    shift += 7;
  } while (b & 0x80);
  return [value, i];
}

/** `wasm` without its custom sections called one of `names`. */
export function dropCustomSections(wasm, names) {
  if (wasm.length < 8 || wasm.readUInt32BE(0) !== 0x0061736d) throw new Error('not a WebAssembly binary');
  const kept = [wasm.subarray(0, 8)];
  let i = 8;
  while (i < wasm.length) {
    const start = i;
    const id = wasm[i];
    const [size, body] = leb(wasm, i + 1);
    i = body + size;
    if (id === 0) {
      const [length, at] = leb(wasm, body);
      if (names.includes(wasm.toString('utf8', at, at + length))) continue;
    }
    kept.push(wasm.subarray(start, i));
  }
  return Buffer.concat(kept);
}
