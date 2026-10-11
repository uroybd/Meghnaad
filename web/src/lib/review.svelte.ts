// tasksh's `review`, in the browser: walk the tasks the `_reviewed` report lists, show each, and mark it reviewed,
// skip it, edit it, change it, finish it or delete it. It is only a client of `POST /api/cli`: every step is an
// ordinary command (`modify reviewed:now`, `done`, `delete`), so undo, hooks and the settings work as they do for
// anything else. tasksh's menu is keybindings, not a shell: a key is the answer, with no Enter after it. Two places
// show it: the console (the prompt is off while it asks, and a bar of buttons stands over it) and a card.
// (src/review.cpp in https://github.com/GothenburgBitFactory/taskshell)

import { runCli, type Answers } from './api';
import { splitWords } from './cmdline';
import { store } from './store.svelte';
import {
  configArgs,
  menuKey,
  partLines,
  REVIEW_FILTER,
  REVIEW_INTRO,
  REVIEW_MENU,
  REVIEW_MODIFY_PROMPT,
  REVIEW_NOTHING,
  REVIEW_SORT,
  reviewEnd,
  setupLines,
  setupQuestion,
  taskshStatus,
  type ReviewKey,
} from './tasksh';
import type { CliResponse, ConfirmResult, Row } from './types';

export type ReviewPhase = 'idle' | 'setup' | 'running' | 'asking' | 'modifying' | 'editing' | 'done';
export type ReviewSurface = 'console' | 'card';

/** Taskwarrior is told these with every step, as tasksh does: no questions, and nothing but the answer. */
const QUIET = ['rc.confirmation:no', 'rc.verbose:nothing'];

/** The choices that change the task: what is sent for each, and what is said when it went through. */
const WRITES: Record<'reviewed' | 'complete' | 'delete', { words: string[]; said: string }> = {
  reviewed: { words: ['modify', 'reviewed:now'], said: 'Marked as reviewed.' },
  complete: { words: ['done'], said: 'Completed.' },
  delete: { words: ['delete'], said: 'Deleted.' },
};

/**
 * A question Taskwarrior asked in the middle of a step, which the person answers before it goes ahead: the usual one
 * is "This is a recurring task. Do you want to modify all pending recurrences of this same task?", which
 * `rc.confirmation:no` does not silence (`recurrence.confirmation` governs it), so tasksh shows it too.
 */
interface Asking {
  words: string[];
  answers: Answers;
  r: ConfirmResult;
  /** Which of the question's items is being asked. */
  index: number;
  /** The keys answered yes so far. */
  yes: string[];
  /** What to do when the step has gone through. */
  then: () => Promise<void>;
}

class ReviewSession {
  phase = $state<ReviewPhase>('idle');
  surface = $state<ReviewSurface>('card');
  /** The task being looked at, as `info` gives it. */
  row = $state<Row | null>(null);
  current = $state(0);
  reviewed = $state(0);
  total = $state(0);
  /** The latest thing said (the card shows it; the console prints each as it is said). */
  message = $state('');
  messageKind = $state<'ok' | 'err'>('ok');
  /** The question being asked: the setup's, or one of Taskwarrior's. */
  question = $state<string | null>(null);
  busy = $state(false);

  #uuids: string[] = [];
  #limit = 0;
  /** What is still to be asked in the setup, and the `config` lines each answer writes. */
  #parts: ('uda' | 'report')[] = [];
  #lines: [string, string][] = [];
  #asking: Asking | null = null;

  /** The console is taking the keys (and its prompt is off, except to type what to change). */
  get captures(): boolean {
    return (
      this.surface === 'console' &&
      (this.phase === 'setup' || this.phase === 'running' || this.phase === 'asking' || this.phase === 'modifying')
    );
  }

  /** What the console says while it is waiting. */
  get hint(): string {
    if (this.phase === 'setup' || this.phase === 'asking') return `${this.question ?? ''} (y/n)`;
    if (this.phase === 'modifying') return `${REVIEW_MODIFY_PROMPT} Enter to apply, Escape to cancel.`;
    return REVIEW_MENU;
  }

  /** Begin a review (`limit` 0 is all of them). Does nothing while one is going. */
  async start({ limit = 0, surface }: { limit?: number; surface: ReviewSurface }): Promise<void> {
    if (this.phase !== 'idle') {
      if (this.phase === 'done') this.close();
      else return void this.#say('A review is already in progress.', 'err');
    }
    this.surface = surface;
    this.#limit = limit;
    this.#uuids = [];
    this.current = this.reviewed = this.total = 0;
    this.row = null;
    this.message = '';
    this.busy = true;
    try {
      if (!store.config) await store.loadConfig();
      const status = taskshStatus(store.config);
      if (status.enabled) return await this.#begin();
      const want = { filter: status.filter ?? REVIEW_FILTER, sort: status.sort ?? REVIEW_SORT };
      this.#lines = setupLines(store.config, want);
      this.#parts = [];
      if (status.uda !== 'ok') this.#parts.push('uda');
      if (status.report !== 'ok') this.#parts.push('report');
      this.phase = 'setup';
      this.#ask();
    } finally {
      this.busy = false;
    }
  }

  /**
   * A key pressed (`Enter` is the usual answer): the menu's choice, or y or n to a question. A key that means
   * nothing here is ignored, as keys are.
   */
  async press(key: string): Promise<void> {
    if (this.busy) return;
    const k = key.toLowerCase();
    if (this.phase === 'setup' && (k === 'y' || k === 'n')) return this.answerSetup(k === 'y');
    if (this.phase === 'asking' && (k === 'y' || k === 'n')) return this.answer(k === 'y');
    if (this.phase === 'running') {
      const choice = menuKey(key === 'Enter' ? '' : key);
      if (choice) return this.act(choice);
    }
  }

  /** What was typed, when what is wanted is typed: the words after `modify`. */
  async respond(input: string): Promise<void> {
    if (this.phase === 'modifying') return this.modify(input);
  }

  /** Yes or no to the setup's question (the UDA, then the report). */
  async answerSetup(yes: boolean): Promise<void> {
    if (this.phase !== 'setup' || this.busy) return;
    if (!yes) {
      this.#say('Review needs the reviewed UDA and the _reviewed report.', 'err');
      return this.#end(false);
    }
    const part = this.#parts.shift()!;
    this.busy = true;
    try {
      for (const line of partLines(this.#lines, part)) {
        const r = (await this.#call(configArgs(line))).result;
        if (r.kind === 'error') {
          this.#say(r.message, 'err');
          return this.#end(false);
        }
      }
      if (this.#parts.length) return this.#ask();
      await store.settingsChanged();
      await this.#begin();
    } catch (e) {
      this.#say(e instanceof Error ? e.message : String(e), 'err');
      this.#end(false);
    } finally {
      this.busy = false;
    }
  }

  /** One of the menu's choices. */
  async act(key: ReviewKey): Promise<void> {
    if (this.phase !== 'running' || this.busy || !this.row) return;
    this.busy = true;
    try {
      if (key === 'quit') return this.#end();
      if (key === 'skip') {
        this.#say('Skipped');
        this.current++;
        return await this.#show();
      }
      if (key === 'edit') {
        // The editor is the form the Edit button opens; closing it carries on (see `editClosed`).
        this.phase = 'editing';
        store.editing = { row: this.row, from: null };
        return;
      }
      if (key === 'modify') {
        this.phase = 'modifying';
        this.#say(REVIEW_MODIFY_PROMPT);
        return;
      }
      const { words, said } = WRITES[key];
      await this.#write(words, async () => {
        this.#say(said);
        this.reviewed++;
        this.current++;
        await this.#show();
      });
    } finally {
      this.busy = false;
    }
  }

  /** The words after `modify` for the task being looked at; nothing cancels. The same task is shown again. */
  async modify(input: string): Promise<void> {
    if (this.phase !== 'modifying' || this.busy) return;
    const words = splitWords(input);
    this.phase = 'running';
    if (!words.length) return this.#say('No changes.');
    this.busy = true;
    try {
      await this.#write(['modify', ...words], async () => {
        this.#say('Modified.');
        await this.#show();
      });
    } finally {
      this.busy = false;
    }
  }

  /** The editor was closed: as in tasksh, the task is marked reviewed and shown again. */
  async editClosed(): Promise<void> {
    if (this.phase !== 'editing') return;
    this.phase = 'running';
    this.busy = true;
    try {
      await this.#write(['modify', 'reviewed:now'], async () => {
        this.#say('Modified.');
        await this.#show();
      });
    } finally {
      this.busy = false;
    }
  }

  /** Yes or no to a question Taskwarrior asked in the middle of a step. */
  async answer(yes: boolean): Promise<void> {
    const a = this.#asking;
    if (this.phase !== 'asking' || !a || this.busy) return;
    if (a.r.ask === 'extras') {
      // One question per item (the recurring series, a dependency to repair); no leaves that part undone and the
      // step itself still goes ahead.
      if (yes) a.yes.push(a.r.items[a.index].key);
      a.index++;
      if (a.index < a.r.items.length) return this.#askNext();
      return this.#resend({ extras: a.yes });
    }
    if (!yes) {
      this.#asking = null;
      this.question = null;
      this.phase = 'running';
      return this.#say('Cancelled.');
    }
    return this.#resend(a.r.ask === 'plain' ? { confirmed: true } : { approved: a.r.items.map((i) => i.key) });
  }

  /** Move a review that is going in the console to the card: the same session and place, drawn with buttons. */
  toCard(): void {
    if (this.surface === 'console' && this.phase !== 'idle' && this.phase !== 'done') this.surface = 'card';
  }

  /** Stop, whatever is going on (the card closed, Escape at the prompt). */
  quit(): void {
    if (this.phase === 'idle') return;
    if (this.phase === 'done' || this.phase === 'setup') return this.close();
    this.#end();
  }

  /** Back to nothing happening. */
  close(): void {
    this.phase = 'idle';
    this.row = null;
    this.question = null;
    this.message = '';
    this.#asking = null;
  }

  #ask() {
    this.question = setupQuestion(this.#parts[0], taskshStatus(store.config));
    this.#say(this.surface === 'console' ? `${this.question} (y/n)` : this.question);
  }

  #askNext() {
    const a = this.#asking!;
    this.question = a.r.ask === 'extras' ? a.r.items[a.index].question : a.r.message;
    this.#say(this.surface === 'console' ? `${this.question} (y/n)` : this.question);
  }

  async #resend(more: Answers) {
    const a = this.#asking!;
    this.#asking = null;
    this.question = null;
    this.phase = 'running';
    this.busy = true;
    try {
      await this.#write(a.words, a.then, { ...a.answers, ...more });
    } finally {
      this.busy = false;
    }
  }

  async #begin() {
    const res = await this.#call([
      'rc.color=off',
      'rc.detection=off',
      'rc._forcecolor=off',
      'rc.verbose=nothing',
      '_reviewed',
    ]);
    const r = res.result;
    if (r.kind !== 'report') {
      this.#say(r.kind === 'error' ? r.message : 'The _reviewed report did not answer with a list.', 'err');
      return this.#end(false);
    }
    this.#uuids = r.rows.map((x) => x.uuid);
    this.total = this.#limit ? Math.min(this.#limit, this.#uuids.length) : this.#uuids.length;
    if (this.total === 0) {
      this.#say(REVIEW_NOTHING);
      return this.#end(false);
    }
    this.#say(REVIEW_INTRO);
    this.phase = 'running';
    this.current = this.reviewed = 0;
    await this.#show();
  }

  /** Show the task at `current`, or finish when they are used up. */
  async #show() {
    while (this.current < this.total && (!this.#limit || this.reviewed < this.#limit)) {
      const uuid = this.#uuids[this.current];
      const r = (await this.#call([uuid, 'info'])).result;
      const task = r.kind === 'info' ? r.tasks[0] : undefined;
      if (task) {
        this.row = task;
        if (this.surface === 'console') {
          store.addEntry(`review [${this.current + 1} of ${this.total}] ${task.description}`, r);
        }
        return;
      }
      // Not there any more (finished or deleted elsewhere since the list was made): on to the next.
      this.current++;
    }
    this.#end();
  }

  /**
   * A command for the task being looked at. When it has gone through, `then` carries on; when Taskwarrior asks
   * something first, the person is asked, and the same command is sent again with the answer.
   */
  async #write(words: string[], then: () => Promise<void>, answers: Answers = {}): Promise<void> {
    const uuid = this.row!.uuid;
    try {
      const res = await runCli({ args: [...QUIET, uuid, ...words], ...answers });
      const r = res.result;
      if (r.kind === 'error') return this.#say(r.message, 'err');
      if (r.kind === 'confirm') {
        this.#asking = { words, answers, r, index: 0, yes: [], then };
        this.phase = 'asking';
        return this.#askNext();
      }
      if (res.wrote) {
        store.written();
        if (store.live) void store.refresh(store.live);
      }
      await then();
    } catch (e) {
      this.#say(e instanceof Error ? e.message : String(e), 'err');
    }
  }

  async #call(args: string[]): Promise<CliResponse> {
    return runCli({ args });
  }

  #end(summary = true) {
    if (summary) this.#say(reviewEnd(this.reviewed, this.total));
    this.#asking = null;
    // Said in the console, there is nothing left to show; the card keeps the summary until it is closed.
    if (this.surface === 'console') this.close();
    else this.phase = 'done';
  }

  #say(text: string, kind: 'ok' | 'err' = 'ok') {
    this.message = text;
    this.messageKind = kind;
    if (this.surface === 'console') {
      store.addEntry('', kind === 'err' ? { kind: 'error', message: text } : { kind: 'text', lines: [text] });
    }
  }
}

export const review = new ReviewSession();
