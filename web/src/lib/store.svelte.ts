import { ApiError, getConfig, runCli, type CliInput } from './api';
import { previewLine, reportArgs, shellQuote } from './cmdline';
import type { TaskRef, Vocab } from './completion';
import type { CliResponse, CliResult, ConfigResponse, ReportMeta, Row } from './types';

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
}

export interface Toast {
  text: string;
  kind: 'ok' | 'err';
}

let nextId = 1;

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

/** Long uuids make the "last command" line unreadable; the first 8 characters resolve the same way. */
export function shortenUuids(s: string): string {
  return s.replace(/\b([0-9a-f]{8})-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b/g, '$1');
}

function titleOf(input: CliInput): string {
  return input.args ? previewLine(input.args) : `task ${input.line ?? ''}`.trim();
}

class Store {
  config = $state<ConfigResponse | null>(null);
  configError = $state<string | null>(null);
  /** The console scrollback (commands that aren't reports). */
  entries = $state<Entry[]>([]);
  /** The live table in the Tasks view: the report currently in focus. */
  live = $state<Entry | null>(null);
  view = $state<'tasks' | 'console'>('tasks');
  /** The report and extra filter in focus. Running a report in the console sets these too. */
  report = $state('next');
  filter = $state('');
  /** JSON of the args `live` was loaded with, so we don't fetch the same thing twice. */
  liveKey = '';
  toast = $state<Toast | null>(null);
  /** The most recent command run, whether typed or driven by the UI. Shown above the prompt. */
  lastCommand = $state('');
  /** The last {HISTORY_MAX} commands, oldest first; what ↑/↓ walks. */
  history = $state<string[]>(typeof localStorage === 'undefined' ? [] : loadHistory());
  projects = $state<string[]>([]);
  tags = $state<string[]>([]);
  tasks = $state<TaskRef[]>([]);
  now = $state(Math.floor(Date.now() / 1000));
  /** Bumped after every successful write, for views that show a single task. */
  rev = $state(0);

  editing = $state<{ row: Row; from: Entry | null } | null>(null);
  /** The add-task form (null = closed). `n` re-mounts it for "add another". */
  adding = $state<{ description: string; n: number } | null>(null);
  /** The task detail drawer. */
  detail = $state<{ uuid: string; from: Entry | null } | null>(null);
  settingsOpen = $state(false);
  notifyOpen = $state(false);

  #toastTimer: ReturnType<typeof setTimeout> | undefined;
  #lastWasLive = false;

  reports = $derived<ReportMeta[]>(this.config?.reports ?? []);
  udas = $derived(Object.values(this.config?.config.udas ?? {}));
  vocab = $derived<Vocab>({
    reports: this.reports.map((r) => ({ name: r.name, description: r.description })),
    projects: this.projects,
    tags: this.tags,
    udas: this.udas,
    tasks: this.tasks,
    contexts: Object.keys(this.config?.config.contexts ?? {}),
  });

  constructor() {
    // Keep relative dates ("due in 2d") fresh without re-fetching.
    if (typeof window !== 'undefined') {
      setInterval(() => (this.now = Math.floor(Date.now() / 1000)), 30_000);
    }
  }

  async loadConfig() {
    try {
      this.config = await getConfig();
      this.configError = null;
    } catch (e) {
      this.configError = e instanceof Error ? e.message : String(e);
    }
  }

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
    try {
      localStorage.setItem(HISTORY_KEY, JSON.stringify(next));
    } catch {
      /* private mode: history just won't persist */
    }
  }

  notify(text: string, kind: Toast['kind'] = 'ok') {
    this.toast = { text, kind };
    clearTimeout(this.#toastTimer);
    this.#toastTimer = setTimeout(() => (this.toast = null), kind === 'err' ? 7000 : 3500);
  }

  /** Learn project, tag and task names from rows we've seen, for completion and helpers. */
  #learn(rows: Row[]) {
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

  async #exec(e: Entry, confirmed = false, recurrence?: boolean): Promise<CliResponse | null> {
    e.loading = true;
    e.failure = null;
    try {
      const res = await runCli({ ...e.input, confirmed, recurrence });
      e.result = res.result;
      if (res.result.kind === 'report') this.#learn(res.result.rows);
      if (res.result.kind === 'info') this.#learn(res.result.tasks);
      if (res.wrote) this.rev++;
      return res;
    } catch (err) {
      e.failure = err instanceof ApiError || err instanceof Error ? err.message : String(err);
      return null;
    } finally {
      e.loading = false;
      e.at = Date.now();
    }
  }

  /** Make `name` + `filter` the report in focus and show it in the Tasks view. */
  focusReport(name: string, filter: string[], from?: Entry) {
    this.report = name;
    this.filter = filter.map(shellQuote).join(' ');
    this.view = 'tasks';
    const args = reportArgs(this.filter, this.report);
    this.liveKey = JSON.stringify(args);
    if (from) {
      // Hand over an already-fetched result instead of fetching it again.
      const live: Entry = { ...from, id: nextId++, input: { args }, title: titleOf({ args }) };
      this.live = live;
    }
  }

  /** Run a command from the console. A report command focuses that report instead of scrolling. */
  async run(input: CliInput): Promise<Entry> {
    const e = $state<Entry>({
      id: nextId++, input, title: titleOf(input), result: null, loading: true, failure: null, at: Date.now(),
    });
    this.entries.push(e);
    this.remember(input.line ?? previewLine(input.args ?? []));
    const res = await this.#exec(e);
    if (res?.command?.report && res.result.kind === 'report') {
      this.entries = this.entries.filter((x) => x.id !== e.id);
      this.focusReport(res.command.name, res.command.filter, e);
    } else if (res?.wrote && this.live) {
      // A write from the console changed what the focused report shows.
      void this.refresh(this.live);
    }
    return e;
  }

  /** Run (or re-run) the Tasks view's live table. */
  async runLive(input: CliInput) {
    this.liveKey = JSON.stringify(input.args ?? input.line);
    if (!this.live) {
      this.live = { id: nextId++, input, title: titleOf(input), result: null, loading: true, failure: null, at: Date.now() };
    }
    this.live.input = input;
    this.live.title = titleOf(input);
    this.remember(input.args ? previewLine(input.args) : (input.line ?? ''), true);
    await this.#exec(this.live);
  }

  /** Re-run an entry (after a change), keeping its place. */
  async refresh(e: Entry) {
    await this.#exec(e);
  }

  /** Answer a confirmation prompt on an entry. */
  async confirm(e: Entry, yes: boolean) {
    if (!yes) {
      e.result = { kind: 'text', lines: ['Cancelled.'] };
      return;
    }
    const res = await this.#exec(e, true);
    if (res?.wrote && this.live) void this.refresh(this.live);
  }

  /** Answer "change the whole recurring series?" on an entry: true = all pending, false = only this task. */
  async answerRecurrence(e: Entry, all: boolean) {
    const res = await this.#exec(e, true, all);
    if (res?.wrote && this.live) void this.refresh(this.live);
  }

  /** A GUI action (done, start, add, ...): run it, report the outcome, refresh what's on screen. */
  async act(
    from: Entry | null,
    args: string[],
    confirmed = false,
    recurrence?: boolean,
  ): Promise<CliResponse | null> {
    if (!confirmed) this.remember(previewLine(args));
    try {
      const res = await runCli({ args, confirmed, recurrence });
      const r = res.result;
      if (r.kind === 'error') this.notify(r.message, 'err');
      else if (r.kind === 'changed') this.notify(r.message);
      else if (r.kind === 'text') this.notify(r.lines.join(' '));
      else if (r.kind === 'confirm') {
        // Ask, then repeat the command with the answer. A recurring-series question has three
        // outcomes (all pending / only this task / cancel), which two browser prompts can express.
        if (typeof window !== 'undefined') {
          if (r.recurrence) {
            if (window.confirm(`${r.message}\n\nOK: change all pending recurrences.`)) return this.act(from, args, true, true);
            if (window.confirm('Change only this task, and leave the other recurrences as they are?')) return this.act(from, args, true, false);
          } else if (!confirmed && window.confirm(r.message)) {
            return this.act(from, args, true);
          }
        }
        this.notify('Nothing was changed.');
      }
      if (res.wrote) {
        this.rev++;
        const targets = new Set([from, this.live].filter((x): x is Entry => !!x));
        await Promise.all([...targets].map((t) => this.refresh(t)));
      }
      return res;
    } catch (err) {
      this.notify(err instanceof Error ? err.message : String(err), 'err');
      return null;
    }
  }

  clear() {
    this.entries = [];
  }

  openDetail(uuid: string, from: Entry | null = null) {
    this.detail = { uuid, from };
  }
}

export const store = new Store();
