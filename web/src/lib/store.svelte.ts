import { ApiError, getConfig, runCli, type Answers, type CliInput } from './api';
import { previewLine, reportArgs, shellQuote } from './cmdline';
import { VIRTUAL_TAGS, type TaskRef, type Vocab } from './completion';
import type { CliResponse, CliResult, ConfigResponse, HookLine, ReportMeta, Row } from './types';

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

function hookText(lines: HookLine[]): string {
  return lines.map((l) => l.text).join(' · ');
}

function hookKind(lines: HookLine[] | undefined): Toast['kind'] {
  return lines?.some((l) => l.kind === 'warn') ? 'err' : 'ok';
}

class Store {
  config = $state<ConfigResponse | null>(null);
  configError = $state<string | null>(null);
  /** The console scrollback (commands that aren't reports). */
  entries = $state<Entry[]>([]);
  /** The live table in the Tasks view: the report currently in focus. */
  live = $state<Entry | null>(null);
  view = $state<'tasks' | 'projects' | 'tags' | 'summary' | 'calendar' | 'burndown' | 'console'>('tasks');
  /** The tag the Tags page should show (`n` changes on every request, so asking for the same tag again still reacts). */
  tagFocus = $state<{ tag: string; n: number } | null>(null);
  /** The filter on each of the Summary, Calendar and Burndown pages (Taskwarrior filter syntax). */
  /** The filter on the Projects and Tags pages. They start as the pending tasks; taking that away brings in finished ones. */
  projectsFilter = $state('status:pending');
  tagsFilter = $state('status:pending');
  summaryFilter = $state('');
  calendarFilter = $state('');
  burndownFilter = $state('');
  burndownPeriod = $state<'daily' | 'weekly' | 'monthly' | 'annual'>('daily');
  /** What each of those pages last asked for. They show it with the same component the console uses. */
  panels = $state<{ summary: Entry | null; calendar: Entry | null; burndown: Entry | null }>({
    summary: null,
    calendar: null,
    burndown: null,
  });
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
  /** Text typed at the prompt and not yet run. */
  promptLine = $state('');
  /** The task detail drawer. */
  detail = $state<{ uuid: string; from: Entry | null } | null>(null);
  settingsOpen = $state(false);
  urgencyOpen = $state(false);
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

  /**
   * The saved settings changed (the taskrc dialog, `config`): read them again, and draw again what is on screen,
   * since they can change what it shows (colours, a UDA, a report, `dateformat`).
   */
  async settingsChanged() {
    await this.loadConfig();
    if (this.live) void this.refresh(this.live);
    this.rev++;
  }

  async loadConfig() {
    try {
      this.config = await getConfig();
      this.configError = null;
      void this.refreshNames();
    } catch (e) {
      this.configError = e instanceof Error ? e.message : String(e);
    }
  }

  /**
   * The project and tag names offered for completion normally come from the tasks on screen. With
   * `complete.all.tags` (or `list.all.projects`) Taskwarrior offers the names of finished tasks too,
   * so ask for them (`_tags`, `_projects`) rather than guess.
   */
  async refreshNames() {
    const on = (k: string) => /^(1|y|yes|on|true)$/i.test((this.config?.config.settings?.[k] ?? '').trim());
    const lines = async (cmd: string): Promise<string[]> => {
      try {
        const r = (await runCli({ args: [cmd] })).result;
        return r.kind === 'text' ? r.lines : [];
      } catch {
        return [];
      }
    };
    const [tags, projects] = await Promise.all([
      on('complete.all.tags') ? lines('_tags') : [],
      on('list.all.projects') ? lines('_projects') : [],
    ]);
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
    try {
      localStorage.setItem(HISTORY_KEY, JSON.stringify(this.history));
    } catch {
      /* private mode */
    }
  }

  /** Show a tag's entry on the Tags page: what a tag chip does wherever it is not toggling a report's filter. */
  showTag(tag: string) {
    this.tagFocus = { tag, n: (this.tagFocus?.n ?? 0) + 1 };
    this.detail = null;
    this.view = 'tags';
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

  async #exec(e: Entry, answers: Answers = {}): Promise<CliResponse | null> {
    e.loading = true;
    e.failure = null;
    e.answers = answers;
    try {
      const res = await runCli({ ...e.input, ...answers });
      e.result = res.result;
      e.feedback = res.feedback;
      if (res.result.kind === 'report') this.#learn(res.result.rows);
      if (res.result.kind === 'info') this.#learn(res.result.tasks);
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

  /** Make `name` + `filter` the report in focus and show it in the Tasks view. */
  focusReport(name: string, filter: string[], from?: Entry) {
    this.report = name;
    this.filter = filter.map(shellQuote).join(' ');
    this.view = 'tasks';
    const args = reportArgs(this.filter, this.report);
    if (from) {
      // Hand over an already-fetched result instead of fetching it again. Only then is the table
      // already showing these arguments; otherwise the Tasks view must run them (marking them as
      // loaded here would make it skip the run and keep showing the previous, unfiltered, table).
      this.liveKey = JSON.stringify(args);
      this.live = { ...from, id: nextId++, input: { args }, title: titleOf({ args }) };
    }
  }

  /** Run a command from the console. A report command focuses that report instead of scrolling. */
  async run(input: CliInput): Promise<Entry> {
    // Typed in the Console, a report prints right there. From the bar under the other pages it opens
    // in the Tasks view instead, where it can be sorted and filtered.
    const inConsole = this.view === 'console';
    const e = $state<Entry>({
      id: nextId++,
      input,
      title: titleOf(input),
      result: null,
      loading: true,
      failure: null,
      at: Date.now(),
    });
    this.entries.push(e);
    this.remember(input.line ?? previewLine(input.args ?? []));
    const res = await this.#exec(e);
    // `config` rewrites the saved settings: pick them up so the pages follow.
    if (res?.command?.name === 'config') {
      if (res.result.kind !== 'error') void this.settingsChanged();
      else this.#redact(e);
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

  /** Run (or re-run) the Tasks view's live table. */
  async runLive(input: CliInput) {
    this.liveKey = JSON.stringify(input.args ?? input.line);
    if (!this.live) {
      this.live = {
        id: nextId++,
        input,
        title: titleOf(input),
        result: null,
        loading: true,
        failure: null,
        at: Date.now(),
      };
    }
    this.live.input = input;
    this.live.title = titleOf(input);
    this.remember(input.args ? previewLine(input.args) : (input.line ?? ''), true);
    await this.#exec(this.live);
  }

  /** Run (or re-run) what one of the Summary, Calendar or Burndown pages shows. */
  async runPanel(which: 'summary' | 'calendar' | 'burndown', input: CliInput) {
    if (!this.panels[which]) {
      this.panels[which] = {
        id: nextId++,
        input,
        title: titleOf(input),
        result: null,
        loading: true,
        failure: null,
        at: Date.now(),
      };
    }
    const e = this.panels[which]!;
    e.input = input;
    e.title = titleOf(input);
    await this.#exec(e);
  }

  /** Re-run an entry (after a change), keeping its place. */
  async refresh(e: Entry) {
    await this.#exec(e);
  }

  /** Answer a plain yes/no question on an entry. */
  async confirm(e: Entry, yes: boolean) {
    if (!yes) {
      e.result = { kind: 'text', lines: ['Cancelled.'] };
      return;
    }
    const res = await this.#exec(e, { ...e.answers, confirmed: true });
    if (res?.wrote && this.live) void this.refresh(this.live);
  }

  /** Answer a per-task question on an entry with the keys that were ticked. */
  async answerItems(e: Entry, keys: string[]) {
    const r = e.result;
    if (r?.kind !== 'confirm') return;
    const more: Answers = r.ask === 'permission' ? { approved: keys } : { extras: keys };
    const res = await this.#exec(e, { ...e.answers, ...more });
    if (res?.wrote && this.live) void this.refresh(this.live);
  }

  /** Something was written: let the views reload, and refresh the completion names if they depend on it. */
  written() {
    this.rev++;
    void this.refreshNames();
  }

  /** The taskrc's `confirmation` (on unless turned off): the delete buttons ask before they delete. */
  get confirmation(): boolean {
    const v = this.config?.config.settings?.confirmation;
    return v === undefined || /^(1|y|yes|on|true)$/i.test(v.trim());
  }

  /** A GUI action (done, start, add, ...): run it, report the outcome, refresh what's on screen. */
  async act(from: Entry | null, args: string[], answers: Answers = {}, again = false): Promise<CliResponse | null> {
    if (!again) this.remember(previewLine(args));
    try {
      const res = await runCli({ args, ...answers });
      const r = res.result;
      // What a hook said goes with the outcome, in the same toast.
      const said = res.feedback?.length ? ` ${hookText(res.feedback)}` : '';
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
            const question = r.items.length === 1 ? r.items[0].question : r.message;
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

  clear() {
    this.entries = [];
  }

  openDetail(uuid: string, from: Entry | null = null) {
    this.detail = { uuid, from };
  }
}

export const store = new Store();
