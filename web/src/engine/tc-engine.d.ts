// The engine's WebAssembly package (crates/browser, built by scripts/build-engine.mjs into src/engine/pkg). Typed by hand
// so that type-checking and the tests do not need it built; keep this in step with crates/browser/src/lib.rs.
declare module 'tc-engine' {
  export default function init(input?: { module_or_path: string | URL | Request }): Promise<unknown>;
  /** The same, from bytes already in hand and at once (the tests, which have no network). */
  export function initSync(input: { module: BufferSource | WebAssembly.Module }): unknown;
  /** Any other helper (`tc_core::helpers::call`): its name and its arguments as JSON text, its answer as JSON text. */
  export function helper(name: string, args: string): string;
  /** A date in a `dateformat` pattern (`tc_core::helpers::format_date`): `ts` is seconds since 1970, `tz` seconds east of UTC. */
  export function format_date(pattern: string, ts: number, tz: number, week_starts_monday: boolean): string;
  export class Engine {
    constructor();
    /** The settings (the `config` of `GET /api/config`), as JSON text. */
    set_config(json: string): void;
    /** Which hooks the saved script defines, as JSON text: only these are called. */
    set_hooks(defined: string, base: string): void;
    /** Go on from what `save` returned; throws if that is unusable. */
    restore(bytes: Uint8Array): void;
    save(): Promise<Uint8Array>;
    /** Changes whenever the tasks do. */
    changes(): number;
    /** Pull and push with the bucket behind the Worker at `base` ('' for this page's own). */
    sync(base: string): Promise<void>;
    /** One command: the JSON text the page reads (`{ error }` if the request is not one), for a request as JSON text. */
    run(request: string): Promise<string>;
    /** An import (or, without `apply`, what it would do): JSON text. */
    import(text: string, apply: boolean, tz: number, now: number): Promise<string>;
  }
}
