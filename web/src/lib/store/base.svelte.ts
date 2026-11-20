import { runCli, type CliInput } from '../api';
import { previewLine } from '../cmdline';
import { VIRTUAL_TAGS, type TaskRef } from '../completion';
import type { SettingsTab } from '../route';
import { saidKind, saidText } from '../said';
import type { Scope } from '../pageContext';
import type { CliResult, HookLine, Row } from '../types';
import type { Answers } from '../api';

export interface Entry {
  id: number;
  input: CliInput;
  /** What was run, as it would be typed. */
  title: string;
  result: CliResult | null;
  loading: boolean;
  /** Transport-level failure (not a command error, which arrives as an `error` result). */
  failure: string | null;
  at: number;
  /** What the hooks printed while this command ran. */
  feedback?: HookLine[];
  /** What was already answered on this command, for the questions that follow the first. */
  answers?: Answers;
  /** The question it asked was answered no. */
  declined?: boolean;
  /** It continues the line above it (a later statement of `a ; b`): the console draws them as one block. */
  chained?: boolean;
  /** The page it belongs to, which may run it in a context of its own; the console's entries have none. */
  scope?: Scope;
}

export interface Toast {
  text: string;
  /** `busy` is something still going on (it stays until the work says how it went). */
  kind: 'ok' | 'err' | 'busy';
}

export type View =
  'tasks' | 'projects' | 'tags' | 'summary' | 'calendar' | 'burndown' | 'boards' | 'kanban' | 'console' | 'settings';

let lastId = 0;

/** A new entry's id, unique for the page's life. */
export const nextId = () => ++lastId;

/** An entry that is about to run `input`. */
export function newEntry(input: CliInput, scope?: Scope): Entry {
  return {
    id: nextId(),
    input,
    title: titleOf(input),
    result: null,
    loading: true,
    failure: null,
    at: Date.now(),
    scope,
  };
}

export function titleOf(input: CliInput): string {
  // A line typed with `task` in front (the engine drops it) is headed by one `task`, not two.
  return input.args ? previewLine(input.args) : `task ${(input.line ?? '').replace(/^task(\s+|$)/, '')}`.trim();
}

export const hookText = (lines: HookLine[]): string => saidText(lines);
export const hookKind = (lines: HookLine[] | undefined): Toast['kind'] => saidKind(lines);

/**
 * What every part of the store shares: where the app is, the live table, the toast, the names offered for completion
 * and the count of writes. The parts build on it in turn (`settings`, `navigation`, `console`, `tasks`); `refresh`, which
 * runs an entry again, is the console's, and is named here because the settings call it.
 */
export abstract class StoreBase {
  view = $state<View>('tasks');
  /** The board the Boards page shows (null: the list of boards). */
  boardId = $state<string | null>(null);
  /** The kanban the Kanban page shows (null: the list of kanbans). */
  kanbanId = $state<string | null>(null);
  /** The report it is shown on in place of its own, for this visit (null: its own). */
  kanbanReport = $state<string | null>(null);
  /** The tab the Settings page shows. */
  settingsTab = $state<SettingsTab>('taskrc');
  /** The file of the taskrc folder the taskrc tab shows (`null`: the main taskrc). */
  settingsFile = $state<string | null>(null);
  /** The task detail drawer. */
  detail = $state<{ uuid: string; from: Entry | null } | null>(null);
  /** The live table in the Tasks view: the report currently in focus. */
  live = $state<Entry | null>(null);
  /** The report and extra filter in focus. Running a report in the console sets these too. */
  report = $state('next');
  filter = $state('');
  /** JSON of the args `live` was loaded with, so we don't fetch the same thing twice. */
  liveKey = '';
  /** The context the live table was last fetched in (`''`: the active one), so a changed choice fetches it again. */
  liveContext = '';
  toast = $state<Toast | null>(null);
  projects = $state<string[]>([]);
  tags = $state<string[]>([]);
  tasks = $state<TaskRef[]>([]);
  now = $state(Math.floor(Date.now() / 1000));
  /** Bumped after every successful write, for views that show a single task. */
  rev = $state(0);

  protected toastTimer: ReturnType<typeof setTimeout> | undefined;

  /** Run an entry again, keeping its place. */
  abstract refresh(e: Entry): Promise<void>;

  notify(text: string, kind: Toast['kind'] = 'ok') {
    this.toast = { text, kind };
    clearTimeout(this.toastTimer);
    this.toastTimer = setTimeout(() => (this.toast = null), kind === 'err' ? 7000 : 3500);
  }

  /**
   * Wait for `work`, which a form is waiting on too. A hook can make it slow, so after `after` ms the form is let go
   * (`release`) and a toast says `text` until the work ends and tells how it went in the usual way. `released` says whether
   * the form was let go, in which case it is for the caller to bring it back if the work was refused.
   */
  async waitOrRelease<T>(
    work: Promise<T>,
    release: () => void,
    text: string,
    after = 1000,
  ): Promise<{ value: T; released: boolean }> {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const slow = new Promise<null>((resolve) => (timer = setTimeout(() => resolve(null), after)));
    const first = await Promise.race([work.then((value) => ({ value })), slow]);
    clearTimeout(timer);
    if (first) return { value: first.value, released: false };
    release();
    clearTimeout(this.toastTimer);
    this.toast = { text, kind: 'busy' };
    try {
      return { value: await work, released: true };
    } finally {
      // The outcome has usually replaced it by now; if the work said nothing, it does not stay.
      if (this.toast?.kind === 'busy' && this.toast.text === text) this.toast = null;
    }
  }

  /**
   * The project and tag names offered for completion (the prompt, the filter bar and the task form) come from
   * the tasks on screen, which is only some of them. Ask for all of them: `_names` is `_tags` and `_projects` in
   * one request (the lists follow `complete.all.tags` and `list.all.projects` as Taskwarrior's completion does).
   */
  async refreshNames() {
    let tags: string[];
    let projects: string[];
    try {
      const r = (await runCli({ args: ['_names'] })).result;
      const v = r.kind === 'json' ? (r.value as { tags?: unknown; projects?: unknown }) : null;
      const names = (x: unknown) => (Array.isArray(x) ? x.filter((n): n is string => typeof n === 'string') : []);
      tags = names(v?.tags);
      projects = names(v?.projects);
    } catch {
      return;
    }
    if (!tags.length && !projects.length) return;
    const virtual = new Set(VIRTUAL_TAGS);
    this.tags = [...new Set([...this.tags, ...tags.filter((t) => !virtual.has(t))])].sort();
    const names = new Set(this.projects);
    for (const p of projects) {
      const parts = p.split('.');
      for (let i = 1; i <= parts.length; i++) names.add(parts.slice(0, i).join('.'));
    }
    this.projects = [...names].sort();
  }

  /** Learn project, tag and task names from rows we've seen, for completion and helpers. */
  protected learn(rows: Row[]) {
    const projects = new Set(this.projects);
    const tags = new Set(this.tags);
    const tasks = new Map(this.tasks.map((t) => [t.uuid, t]));
    for (const r of rows) {
      if (r.project) {
        // Offer every ancestor too: Home.Kitchen implies Home.
        const parts = r.project.split('.');
        for (let i = 1; i <= parts.length; i++) projects.add(parts.slice(0, i).join('.'));
      }
      r.tags.forEach((t) => tags.add(t));
      tasks.set(r.uuid, { id: r.id, uuid: r.uuid, description: r.description });
    }
    this.projects = [...projects].sort();
    this.tags = [...tags].sort();
    this.tasks = [...tasks.values()];
  }

  /** Something was written: let the views reload, and refresh the completion names if they depend on it. */
  written() {
    this.rev++;
    void this.refreshNames();
  }
}
