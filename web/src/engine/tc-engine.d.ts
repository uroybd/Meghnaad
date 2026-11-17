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
  /** Taken by `fetch`. */
  export class Source {
    private constructor();
    free(): void;
  }
  /** Taken by `Engine.sync`. */
  export class Fetch {
    private constructor();
    free(): void;
  }
  /** The static flavour's branch, for reading the settings with no engine held. */
  export class Reader {
    private constructor();
    free(): void;
    /** A `GET` of a settings route: `{ status, type, body }` as JSON text, as `Engine.api` answers it. */
    get(path: string): Promise<string>;
  }
  /** Ask the store what is new since the source's version, with no engine held. */
  export function fetch(source: Source): Promise<Fetch>;
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
    /** Where a sync goes and the version it starts from, for `fetch` to run while the engine answers commands. */
    source(base: string): Source;
    /** Pull and push: with what `fetch` found (the store is asked only for what it could not answer), or, without
     * it, with the store (the git branch once connected, else the bucket behind the Worker at `base`, '' for this
     * page's own). */
    sync(base: string, fetched?: Fetch): Promise<void>;
    /** One command: the JSON text the page reads (`{ error }` if the request is not one), for a request as JSON text. */
    run(request: string): Promise<string>;
    /** An import (or, without `apply`, what it would do): JSON text. */
    import(text: string, apply: boolean, tz: number, now: number): Promise<string>;

    // The static flavour's build only (`--static`, the `git` feature).
    /** The salt of the key on this branch (`meta`'s), written with a new one if this is the first client. */
    static git_salt(api: string, repo: string, branch: string, token: string): Promise<Uint8Array>;
    /** Sync with this branch from now on, with this key (32 bytes). */
    connect_git(api: string, repo: string, branch: string, token: string, key: Uint8Array): void;
    /** One of the settings routes, answered from the branch: `{ status, type, body }` as JSON text. */
    api(method: string, path: string, body: Uint8Array): Promise<string>;
    /** The branch, to read a setting from while the engine is busy. */
    reader(): Reader;
    /** The hook secrets, opened, for the hooks module (`{"NAME": {"value", "hosts"}}`); empty when there are none. */
    hook_secrets(): Promise<Uint8Array>;
  }
}

// The static flavour's hooks module (crates/browser-hooks): the Lua runner, loaded when a hook runs.
declare module 'tc-hooks' {
  export default function init(input?: { module_or_path: string | URL | Request }): Promise<unknown>;
  /** `POST /api/hooks/check`'s answer: `{ ok, defined }` or `{ ok: false, line, message }`, as JSON text. */
  export function check(source: string): string;
  export class Hooks {
    /** The script, and the secrets as `hook_secrets` gives them. */
    constructor(source: string, secrets: Uint8Array);
    /** One hook call, `POST /api/run`'s body: its answer as JSON text. */
    run(body: string): Promise<string>;
    free(): void;
  }
}
