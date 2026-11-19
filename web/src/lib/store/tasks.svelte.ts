import { runCli, type Answers, type CliInput } from '../api';
import { previewLine } from '../cmdline';
import { runSequence, type Joiner } from '../compound';
import { toggled } from '../markdown';
import { confirmText } from '../said';
import type { CliResponse } from '../types';
import { hookKind, hookText, newEntry, titleOf, type Entry } from './base.svelte';
import { ConsoleStore } from './console.svelte';

type Panel = 'summary' | 'calendar' | 'burndown';

/** What the pages show and the changes their buttons make: the live table, the pages' panels, questions and actions. */
export abstract class TasksStore extends ConsoleStore {
  /** What each of the Summary, Calendar and Burndown pages last asked for. They show it with the same component the console uses. */
  panels = $state<Record<Panel, Entry | null>>({ summary: null, calendar: null, burndown: null });

  #ticking = false;

  /** Run (or re-run) the Tasks view's live table. */
  async runLive(input: CliInput) {
    this.liveKey = JSON.stringify(input.args ?? input.line);
    this.liveContext = this.contextChoice('tasks') ?? '';
    this.live ??= newEntry(input, 'tasks');
    this.live.input = input;
    this.live.title = titleOf(input);
    this.remember(input.args ? previewLine(input.args) : (input.line ?? ''), true);
    await this.exec(this.live);
  }

  /** Run (or re-run) what one of the Summary, Calendar or Burndown pages shows. */
  async runPanel(which: Panel, input: CliInput) {
    this.panels[which] ??= newEntry(input, which);
    const e = this.panels[which];
    e.input = input;
    e.title = titleOf(input);
    await this.exec(e);
  }

  /** Answer a plain yes/no question on an entry. */
  async confirm(e: Entry, yes: boolean) {
    if (!yes) {
      e.result = { kind: 'text', lines: ['Cancelled.'] };
      e.declined = true;
      this.wake(e);
      return;
    }
    const res = await this.exec(e, { ...e.answers, confirmed: true });
    this.wake(e);
    if (res?.wrote && this.live) void this.refresh(this.live);
  }

  /** Answer a per-task question on an entry with the keys that were ticked. */
  async answerItems(e: Entry, keys: string[]) {
    const r = e.result;
    if (r?.kind !== 'confirm') return;
    const more: Answers = r.ask === 'permission' ? { approved: keys } : { extras: keys };
    if (r.ask === 'permission' && !keys.length) e.declined = true;
    const res = await this.exec(e, { ...e.answers, ...more });
    this.wake(e);
    if (res?.wrote && this.live) void this.refresh(this.live);
  }

  /**
   * Tick or untick a checkbox in a string UDA. The engine changes the text (`tc_core::markdown`), and the task is told the
   * new one with an ordinary `modify <uda>:<text>`. One at a time, so a second click never works from the text the first
   * one has just changed. It changes only the task clicked (`rc.recurrence.confirmation:no`): a box is ticked on this
   * occurrence, so a recurring task is not asked whether the rest of its series should follow. Editing the field in the
   * form still asks.
   */
  async tickBox(from: Entry | null, uuid: string, uda: string, text: string, line: number) {
    if (this.#ticking) return;
    const next = toggled(text, line);
    if (next === null) return;
    this.#ticking = true;
    try {
      await this.act(from, ['rc.recurrence.confirmation:no', uuid, 'modify', `${uda}:${next}`], {}, true);
    } finally {
      this.#ticking = false;
    }
  }

  /** A GUI action (done, start, add, ...): run it, report the outcome, refresh what's on screen. */
  async act(from: Entry | null, args: string[], answers: Answers = {}, again = false): Promise<CliResponse | null> {
    if (!again) this.remember(previewLine(args));
    try {
      const res = await runCli({ args, ...answers });
      const r = res.result;
      // What the command printed besides (a dependency reminder) and what a hook said go with the outcome, in the same
      // toast, a line each; a warning among them makes it one.
      const said = res.feedback?.length ? `\n${hookText(res.feedback)}` : '';
      if (r.kind === 'error') this.notify(r.message + said, 'err');
      else if (r.kind === 'changed') this.notify(r.message + said, hookKind(res.feedback));
      else if (r.kind === 'text') this.notify(r.lines.join(' ') + said);
      else if (r.kind === 'confirm') {
        // Ask, then repeat the command with the answer. A GUI action touches one task, so this is
        // one question; a longer list (rare) is answered as a whole.
        if (typeof window !== 'undefined') {
          const retry = (more: Answers) => this.act(from, args, { ...answers, ...more }, true);
          if (r.ask === 'plain') {
            if (!answers.confirmed && window.confirm(r.message)) return retry({ confirmed: true });
          } else {
            const keys = r.items.map((i) => i.key);
            // With what `task` prints before it (the dependency reminder).
            const question = confirmText(r);
            if (window.confirm(question)) return retry(r.ask === 'permission' ? { approved: keys } : { extras: keys });
            // A follow-up can be declined while the change itself still goes ahead.
            if (r.ask === 'extras' && window.confirm('Go ahead with the change without that?'))
              return retry({ extras: [] });
          }
        }
        this.notify('Nothing was changed.');
      }
      if (res.wrote) {
        this.written();
        const targets = new Set([from, this.live].filter((x): x is Entry => !!x));
        await Promise.all([...targets].map((t) => this.refresh(t)));
      }
      return res;
    } catch (err) {
      this.notify(err instanceof Error ? err.message : String(err), 'err');
      return null;
    }
  }

  /**
   * A button that stands for a line of statements (a task command with `&&`, `||` or `;`): each runs as `act` would
   * run it, and it succeeds when it was neither refused nor left unanswered.
   */
  async actSteps(from: Entry | null, steps: { args: string[]; join: Joiner | null }[]): Promise<boolean> {
    return runSequence(
      steps.map((s) => s.join),
      async (i) => {
        const res = await this.act(from, steps[i].args);
        return !!res && res.result.kind !== 'error' && res.result.kind !== 'confirm';
      },
    );
  }
}
