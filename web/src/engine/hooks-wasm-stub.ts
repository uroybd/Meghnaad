// What `tc-hooks-wasm?url` is in a Worker flavour's build, which runs hooks on the Worker: no file. A module of our own,
// not a `data:` URL, which the engine's Web Worker could not load (an `application/wasm` import is read as WebAssembly,
// and an empty one is none), and the engine did not start.
export default '';
