import { ApiError, runCli, type Answers, type CliInput } from '../api';
import { previewLine } from '../cmdline';
import { runSequence, type Statement } from '../compound';
import { withContext } from '../pageContext';
import type { CliResponse, CliResult } from '../types';
import { hookKind, hookText, nextId, titleOf, type Entry } from './base.svelte';
import { NavigationStore } from './navigation.svelte';

const HISTORY_KEY = 'tw-web-history';
export const HISTORY_MAX = 20;

function loadHistory(): string[] {
  try {
    const v = JSON.parse(localStorage.getItem(HISTORY_KEY) ?? '[]');
    return Array.isArray(v) ? v.filter((x) => typeof x === 'string').slice(-HISTORY_MAX) : [];
  } catch {
    return [];
  }
}

function saveHistory(history: string[]) {
  try {
    localStorage.setItem(HISTORY_KEY, JSON.stringify(history));
  } catch {
    /* private mode: history just won't persist */
  }
}

/** Long uuids make the "last command" line unreadable; the first 8 characters resolve the same way. */
export function shortenUuids(s: string): string {
  return s.replace(/\b([0-9a-f]{8})-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b/g, '$1');
}

/** The console: its entries, the history, the prompt, and running a command or a line of them. */
export abstract class ConsoleStore extends NavigationStore {
  /** The console scrollback (commands that aren't reports). */
  entries = $state<Entry[]>([]);
  /** The most recent command run, whether typed or driven by the UI. Shown above the prompt. */
  lastCommand = $state('');
  /** The last {HISTORY_MAX} commands, oldest first; what ↑/↓ walks. */
  history = $state<string[]>(typeof localStorage === 'undefined' ? [] : loadHistory());
  /** Text typed at the prompt and not yet run. */
  promptLine = $state('');
  /** A line of several statements is running: the prompt is back when the last of them has ended. */
  chainBusy = $state(false);
  /** Ask the prompt to take this text and the focus (the bulk bar's Command); `n` makes each ask a new one. */
  promptRequest = $state<{ text: string; n: number } | null>(null);

  #waiting = new Map<number, () => void>();
  #aborted = false;
  /** The heading of the line that runs, once an entry has shown it. */
  #heading: { text: string; shown: boolean } = { text: '', shown: false };
  #lastWasLive = false;

  /**
   * Record a command as "last run" and in the history.
   * Live-table updates (typing a filter) are `replaceable`: each replaces the previous one, so
   * half-typed filters don't flood the 20 slots.
   */
  remember(cmd: string, replaceable = false) {
    const text = cmd.replace(/^task\s+/, '').trim();
    if (!text) return;
    this.lastCommand = shortenUuids(`task ${text}`);
    // Only a live update replaces the previous live update; a real command keeps it.
    const h = replaceable && this.#lastWasLive ? this.history.slice(0, -1) : this.history;
    const next = [...h.filter((x) => x !== text), text].slice(-HISTORY_MAX);
    this.#lastWasLive = replaceable;
    this.history = next;
    saveHistory(next);
  }

  /**
   * A refused `config` line may have carried a secret. Keep the command and the name, drop the value from
   * everywhere the browser remembers it: the scrollback, the "last command" line and the saved history.
   */
  #redact(e: Entry) {
    const typed = e.input.line ?? previewLine(e.input.args ?? []);
    const text = typed.replace(/^task\s+/, '').trim();
    const words = text.split(/\s+/);
    const at = words.indexOf('config');
    const safe = at < 0 || words.length <= at + 2 ? text : [...words.slice(0, at + 2), '…'].join(' ');
    e.title = `task ${safe}`;
    e.input = { line: safe };
    this.lastCommand = e.title;
    this.history = this.history.map((x) => (x === text ? safe : x));
    saveHistory(this.history);
  }

  /** Run an entry's command (with the answers given so far) and put what came back in it. */
  protected async exec(e: Entry, answers: Answers = {}): Promise<CliResponse | null> {
    e.loading = true;
    e.failure = null;
    e.answers = answers;
    try {
      const res = await runCli({ ...withContext(e.input, e.scope && this.contextChoice(e.scope)), ...answers });
      e.result = res.result;
      e.feedback = res.feedback;
      if (res.result.kind === 'report') this.learn(res.result.rows);
      if (res.result.kind === 'info') this.learn(res.result.tasks);
      if (res.wrote) this.written();
      return res;
    } catch (err) {
      e.failure = err instanceof ApiError || err instanceof Error ? err.message : String(err);
      return null;
    } finally {
      e.loading = false;
      e.at = Date.now();
    }
  }

  /** Re-run an entry (after a change), keeping its place. */
  async refresh(e: Entry) {
    await this.exec(e);
  }

  /**
   * Print something into the console that is not a command's answer (a review's banner and messages). An empty
   * `title` prints no `$` line above it.
   */
  addEntry(title: string, result: CliResult): Entry {
    const e = $state<Entry>({
      id: nextId(),
      input: { args: [] },
      title,
      result,
      loading: false,
      failure: null,
      at: Date.now(),
    });
    this.entries.push(e);
    return e;
  }

  /**
   * Run a command from the console. A report command focuses that report instead of scrolling. `typed` is what was
   * typed when `input` is that with a task command written out: the history keeps it.
   */
  async run(input: CliInput, typed?: string, chain?: { heading: string | null }): Promise<Entry> {
    // Typed in the Console, a report prints right there. From the bar under the other pages it opens
    // in the Tasks view instead, where it can be sorted and filtered.
    const inConsole = this.view === 'console';
    const e = $state<Entry>({
      id: nextId(),
      input,
      title: chain ? (chain.heading ?? '') : titleOf(input),
      result: null,
      loading: true,
      failure: null,
      at: Date.now(),
      chained: chain !== undefined && chain.heading === null,
    });
    this.entries.push(e);
    if (!chain) this.remember(typed ?? input.line ?? previewLine(input.args ?? []));
    const res = await this.exec(e);
    // `config` and `context` rewrite the saved settings: pick them up so the pages follow.
    const name = res?.command?.name;
    if (name === 'config' || name === 'context') {
      if (res?.result.kind !== 'error') void this.settingsChanged();
      else if (name === 'config') this.#redact(e);
    }
    if (res?.command?.report && res.result.kind === 'report' && !inConsole) {
      this.entries = this.entries.filter((x) => x.id !== e.id);
      this.focusReport(res.command.name, res.command.filter, e);
      // The entry that would have shown them is gone.
      if (res.feedback?.length) this.notify(hookText(res.feedback), hookKind(res.feedback));
    } else if (res?.wrote && this.live) {
      // A write from the console changed what the focused report shows.
      void this.refresh(this.live);
    }
    return e;
  }

  /**
   * Run the statements of a line one after the other, as a shell does (`&&` after a success, `||` after a failure,
   * `;` always). Each is an entry of its own in the Console, under one heading, the line as it was typed (which is also
   * what the history keeps). A statement
   * that asks a question waits for the answer: yes goes on, no is a failure. Clearing the Console ends the line.
   */
  async runChain(steps: Statement[], typed: string) {
    if (this.chainBusy) return;
    this.chainBusy = true;
    this.#aborted = false;
    this.remember(typed);
    this.#heading = { text: titleOf({ line: typed }), shown: false };
    try {
      await runSequence(
        steps.map((s) => s.join),
        async (i) => !this.#aborted && (await this.#runStep(steps[i].line)),
      );
    } finally {
      this.chainBusy = false;
    }
  }

  /** One statement of a line: did it succeed? */
  async #runStep(line: string): Promise<boolean> {
    if (line === 'clear' || line === 'cls') {
      this.entries = [];
      this.#heading.shown = false; // what comes after it is a new block
      return true;
    }
    const heading = this.#heading.shown ? null : this.#heading.text;
    this.#heading.shown = true;
    const e = await this.run({ line }, undefined, { heading });
    while (e.result?.kind === 'confirm' && !this.#aborted) {
      await new Promise<void>((resume) => this.#waiting.set(e.id, resume));
    }
    return !this.#aborted && !e.failure && !e.declined && e.result?.kind !== 'error';
  }

  /** A question was answered (or the line was abandoned): the statement that waits for it looks again. */
  protected wake(e: Entry) {
    this.#waiting.get(e.id)?.();
    this.#waiting.delete(e.id);
  }

  clear() {
    this.entries = [];
    // A line that waits for an answer on an entry that is gone ends there.
    this.#aborted = true;
    for (const wake of this.#waiting.values()) wake();
    this.#waiting.clear();
  }

  /** Put `text` at the prompt, with the cursor after it, ready to be carried on. */
  requestPrompt(text: string) {
    this.promptRequest = { text, n: (this.promptRequest?.n ?? 0) + 1 };
  }
}
