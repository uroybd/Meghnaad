// What the settings screens read, asked for once and shared (spec 0023). Each screen used to fetch for itself, again on
// every change of the settings, so one save on the settings page read the same files several times over; on the static
// web app each read is a request to the forge's API. Here every read is an entry: callers at the same moment share one
// request, an answer is good for a short while (switching tabs asks for nothing), a save puts what it saved in its
// entry, and a change made elsewhere marks the entries stale and bumps `rev`, which the screens follow.

import {
  deleteRcFile as apiDeleteRcFile,
  deleteSecret as apiDeleteSecret,
  getHooks,
  getRcFile,
  getRcFiles,
  getSecrets,
  getTaskrcText,
  importRcFiles as apiImportRcFiles,
  putHooks as apiPutHooks,
  putRcFile as apiPutRcFile,
  putSecret as apiPutSecret,
  type SecretInfo,
} from './api';
import type { RcFiles, TaskrcResponse } from './types';
import { ROOT } from './rcpaths';

/** How long an answer is used again without asking: long enough for a burst and for moving between tabs, short enough
 * that a change made on another device shows up soon. */
export const FRESH_MS = 30_000;

class Entry<T> {
  value = $state<T | undefined>(undefined);
  private at = 0;
  private gen = 0;
  private asking: Promise<T> | undefined;

  constructor(private readonly fetcher: () => Promise<T>) {}

  /** The value, read if there is none, or it is stale or old; a read on its way is joined. */
  get(): Promise<T> {
    if (this.value !== undefined && this.at > 0 && Date.now() - this.at < FRESH_MS) return Promise.resolve(this.value);
    if (this.asking) return this.asking;
    const gen = this.gen;
    const asking = this.fetcher().then((v) => {
      // Marked stale while on its way: what came back may already be old, so it is kept but read again next time.
      this.set(v);
      if (gen !== this.gen) this.at = 0;
      return v;
    });
    this.asking = asking;
    void asking.then(
      () => (this.asking = undefined),
      () => (this.asking = undefined),
    );
    return asking;
  }

  /** What a save answered with, taken as read. */
  set(v: T) {
    this.value = v;
    this.at = Date.now();
  }

  /** Read again next time (the value stays, for whoever shows it meanwhile). */
  stale() {
    this.at = 0;
    this.gen++;
    this.asking = undefined;
  }
}

class SettingsData {
  /** Bumped when settings changed somewhere else; a screen showing them reads them again. */
  rev = $state(0);
  readonly files = new Entry<RcFiles>(getRcFiles);
  readonly hooks = new Entry<string>(getHooks);
  readonly secrets = new Entry<SecretInfo[]>(getSecrets);
  #files = new Map<string, Entry<string>>();

  /** One file of the taskrc folder, the main taskrc included. */
  file(path: string): Entry<string> {
    let e = this.#files.get(path);
    if (!e) {
      e = new Entry(() => (path === ROOT ? getTaskrcText() : getRcFile(path)));
      this.#files.set(path, e);
    }
    return e;
  }

  /**
   * The settings changed (a save, `config` in the console, another tab, the branch): everything is read again when
   * next shown, except the files in `fresh`, which the caller has just saved and holds the answer for.
   */
  changed(fresh: string[] = []) {
    this.files.stale();
    for (const [path, e] of this.#files) if (!fresh.includes(path)) e.stale();
    this.rev++;
  }

  /** The hooks changed elsewhere. */
  hooksChanged() {
    this.hooks.stale();
    this.secrets.stale();
    this.rev++;
  }

  async putRcFile(path: string, text: string): Promise<TaskrcResponse> {
    const r = await apiPutRcFile(path, text);
    this.file(path).set(r.text);
    this.files.stale();
    return r;
  }

  async deleteRcFile(path: string): Promise<TaskrcResponse> {
    const r = await apiDeleteRcFile(path);
    this.#files.delete(path);
    this.files.stale();
    return r;
  }

  async importRcFiles(files: { path: string; text: string }[], mode: 'merge' | 'replace'): Promise<TaskrcResponse> {
    const r = await apiImportRcFiles(files, mode);
    if (mode === 'replace') this.#files.clear();
    else for (const f of files) this.file(f.path).stale();
    this.files.stale();
    return r;
  }

  async putHooks(text: string): Promise<string[]> {
    const defined = await apiPutHooks(text);
    this.hooks.set(text);
    return defined;
  }

  async putSecret(name: string, value: string, hosts: string[]): Promise<void> {
    await apiPutSecret(name, value, hosts);
    this.secrets.stale();
  }

  async deleteSecret(name: string): Promise<void> {
    await apiDeleteSecret(name);
    this.secrets.stale();
  }

  /** Start again from nothing (a test, or a page that forgot the device). */
  reset() {
    this.files.stale();
    this.hooks.stale();
    this.secrets.stale();
    this.#files.clear();
  }
}

export const settingsData = new SettingsData();
