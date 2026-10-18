import { engine } from '../engine/client.svelte';
import { loadChoices, saveChoices, withContext, NO_CONTEXT, type Scope } from './pageContext';
import { ApiError, getAppSettings, getConfig, putAppSettings, runCli, type Answers, type CliInput } from './api';
import {
  MAX_SETTINGS_BYTES,
  parseSettings,
  serializeBoards,
  serializeHomepage,
  type Homepage,
  type ParsedSettings,
  type TaskCommand,
} from './appsettings';
import type { Board } from './boards';
import { lucide } from './lucideLib.svelte';
import { isHiddenReport, offeredReports as pickable } from './reportList';
import { parsePath, placePath, prefixFilter, taskPath, type Place, type SettingsTab } from './route';
import { previewLine, reportArgs, shellQuote } from './cmdline';
import { VIRTUAL_TAGS, type TaskRef, type Vocab } from './completion';
import { runSequence, type Joiner, type Statement } from './compound';
import { toggled } from './markdown';
import type { FormSeed } from './addPreview';
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
  /** The question it asked was answered no. */
  declined?: boolean;
  /** It continues the line above it (a later statement of `a ; b`): the console draws them as one block. */
  chained?: boolean;
  /** The page it belongs to, which may run it in a context of its own; the console's entries have none. */
  scope?: Scope;
}

export interface Toast {
  text: string;
  kind: 'ok' | 'err';
}

let nextId = 1;

const HISTORY_KEY = 'tw-web-history';
const SHOW_HIDDEN_KEY = 'tw-web-show-hidden-reports';
export const HISTORY_MAX = 20;

function readFlag(key: string): boolean {
  try {
    return localStorage.getItem(key) === '1';
  } catch {
    return false;
  }
}

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
  // A line typed with `task` in front (the engine drops it) is headed by one `task`, not two.
  return input.args ? previewLine(input.args) : `task ${(input.line ?? '').replace(/^task(\s+|$)/, '')}`.trim();
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
  view = $state<
    'tasks' | 'projects' | 'tags' | 'summary' | 'calendar' | 'burndown' | 'boards' | 'console' | 'settings'
  >('tasks');
  /** The board the Boards page shows (null: the list of boards). */
  boardId = $state<string | null>(null);
  /** The tab the Settings page shows. */
  settingsTab = $state<SettingsTab>('taskrc');
  /** The app's own `settings.json`, read (null until it has been). */
  appSettings = $state<ParsedSettings | null>(null);
  /** The tag the Tags page should show (`n` changes on every request, so asking for the same tag again still reacts). */
  tagFocus = $state<{ tag: string; n: number } | null>(null);
  /** The same for a project on the Projects page. */
  projectFocus = $state<{ project: string; n: number } | null>(null);
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
  /** The context the live table was last fetched in (`''`: the active one), so a changed choice fetches it again. */
  liveContext = '';
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
  /**
   * The add-task dialog (null = closed): `quick` is the one line with a preview of the task, otherwise the full form, which
   * starts from `seed` when the line was broken down first. `n` re-mounts it.
   */
  adding = $state<{ n: number; quick: boolean; line: string; seed?: FormSeed } | null>(null);
  /** Text typed at the prompt and not yet run. */
  promptLine = $state('');
  /** A line of several statements is running: the prompt is back when the last of them has ended. */
  chainBusy = $state(false);
  #waiting = new Map<number, () => void>();
  #aborted = false;
  /** The heading of the line that runs, once an entry has shown it. */
  #heading: { text: string; shown: boolean } = { text: '', shown: false };
  /** Ask the prompt to take this text and the focus (the bulk bar's Command); `n` makes each ask a new one. */
  promptRequest = $state<{ text: string; n: number } | null>(null);
  /** The task detail drawer. */
  detail = $state<{ uuid: string; from: Entry | null } | null>(null);
  notifyOpen = $state(false);

  #toastTimer: ReturnType<typeof setTimeout> | undefined;
  #lastWasLive = false;

  /** The task commands in `settings.json`: words that stand for a command line, and buttons that run it. */
  taskCommands = $derived<TaskCommand[]>(this.appSettings?.commands ?? []);
  /** The boards in `settings.json`: pages of numbers, tables and charts. */
  boards = $derived<Board[]>(this.appSettings?.boards ?? []);
  reports = $derived<ReportMeta[]>(this.config?.reports ?? []);
  /** Reports named `_something` are hidden helpers: left out of the lists to pick from unless this is on. */
  showHiddenReports = $state(readFlag(SHOW_HIDDEN_KEY));
  /** The reports to pick from (the sidebar, the Tasks page's report box). */
  offeredReports = $derived(pickable(this.reports, this.showHiddenReports, this.report));
  /** Whether there are any to show or hide, so the switch is only there when it does something. */
  hasHiddenReports = $derived(this.reports.some((r) => isHiddenReport(r.name)));

  setShowHiddenReports(on: boolean) {
    this.showHiddenReports = on;
    try {
      localStorage.setItem(SHOW_HIDDEN_KEY, on ? '1' : '0');
    } catch {
      /* private mode: the choice lasts until the page is closed */
    }
  }

  /** The context each page chose (a name, or `none`); a page not here follows the active one. */
  contextChoices = $state<Record<string, string>>(typeof localStorage === 'undefined' ? {} : loadChoices());

  /** What `scope` chose, or `undefined` to follow the active context. A context that is gone no longer counts. */
  contextChoice(scope: Scope): string | undefined {
    const c = this.contextChoices[scope];
    if (!c || c === NO_CONTEXT) return c;
    const known = this.config?.config.contexts;
    return !known || c in known ? c : undefined;
  }

  setContextChoice(scope: Scope, choice: string | undefined) {
    const next = { ...this.contextChoices };
    if (choice) next[scope] = choice;
    else delete next[scope];
    this.contextChoices = next;
    saveChoices(next);
  }

  /** The context in force on a page: its own choice, else the active one; `null` for none. */
  contextIn(scope: Scope): string | null {
    const c = this.contextChoice(scope) ?? this.config?.config.active_context ?? null;
    return c === NO_CONTEXT ? null : c;
  }

  /** The context that is active for the console, and for every page that follows it. */
  activeContext = $derived(this.config?.config.active_context ?? null);

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
      // What another device or tab did arrives with a sync; draw again what is on screen.
      engine.onChanged(() => {
        this.written();
        if (this.live) void this.refresh(this.live);
      });
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

  /** The icons the task commands use are kept in the browser, and only those. */
  rememberIcons() {
    lucide.keep(this.taskCommands.flatMap((c) => (c.icon ? [c.icon] : [])));
  }

  /** Read `settings.json`. A failure leaves what was there: the commands are a convenience, not needed to work. */
  async loadAppSettings() {
    try {
      this.appSettings = parseSettings(JSON.stringify(await getAppSettings()));
      this.rememberIcons();
    } catch {
      this.appSettings ??= parseSettings('');
    }
  }

  /** Where the app opens: a report or a board of `settings.json`, or `next` when it says nothing. */
  homepage = $derived<Homepage | null>(this.appSettings?.homepage ?? null);

  /**
   * Open the home page the settings choose. Unforced, only from the front page: a person who has already gone somewhere (or
   * came by the address of a task) is left there, and nothing chosen is `next`, which is where the app begins. Forced, it is
   * where the front-page address (`/`) goes whatever was shown. A report or board that is not there any more is no choice.
   */
  goHome(force = false) {
    if (!force && (this.view !== 'tasks' || this.detail || !this.homepage)) return;
    // Going home leaves a task's drawer.
    if (force) this.detail = null;
    const home = this.homepage;
    if (home && 'board' in home && this.boards.some((b) => b.id === home.board)) {
      this.boardId = home.board;
      this.view = 'boards';
    } else if (home && 'report' in home && this.reports.some((r) => r.name === home.report)) {
      this.report = home.report;
      this.view = 'tasks';
    } else if (force) {
      this.report = 'next';
      this.view = 'tasks';
    }
  }

  /** Where the app is, as an address names it (`placePath`): the task in the drawer if one is open, else the page. */
  place = $derived<Place>(
    this.detail
      ? { page: 'task', prefix: this.detail.uuid.slice(0, 8) }
      : this.view === 'tasks'
        ? { page: 'tasks', report: this.report }
        : this.view === 'boards'
          ? { page: 'boards', board: this.boardId }
          : this.view === 'settings'
            ? { page: 'settings', tab: this.settingsTab }
            : { page: this.view },
  );

  /** Save where the app opens (`null`: `next`), the rest of `settings.json` as it was. Throws what the server refused. */
  async saveHomepage(homepage: Homepage | null) {
    await putAppSettings(serializeHomepage(this.appSettings?.raw ?? {}, homepage));
    await this.loadAppSettings();
  }

  /** Save the boards (the rest of `settings.json` as it was) and read them back. Throws what the server refused. */
  async saveBoards(boards: Board[], homepage?: Homepage | null) {
    const text = serializeBoards(this.appSettings?.raw ?? {}, boards, homepage);
    if (new Blob([text]).size > MAX_SETTINGS_BYTES)
      throw new Error(`The settings would be larger than ${MAX_SETTINGS_BYTES / 1024} KB.`);
    await putAppSettings(text);
    await this.loadAppSettings();
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

  /** Open the Settings page, on one of its tabs. */
  openSettings(tab: SettingsTab = this.settingsTab) {
    this.settingsTab = tab;
    this.view = 'settings';
  }

  /** Open the one-line add, with `line` typed in it already. */
  quickAdd(line = '') {
    this.adding = { n: (this.adding?.n ?? 0) + 1, quick: true, line };
  }

  /** Open the full add form, starting from the fields of a line already broken down (or empty). */
  addWithForm(seed?: FormSeed) {
    this.adding = { n: (this.adding?.n ?? 0) + 1, quick: false, line: '', seed };
  }

  /** Show a tag's entry on the Tags page: what a tag chip does wherever it is not toggling a report's filter. */
  showTag(tag: string) {
    this.tagFocus = { tag, n: (this.tagFocus?.n ?? 0) + 1 };
    this.detail = null;
    this.view = 'tags';
  }

  /** Show a project's entry on the Projects page (a sub-project opens its parents too). */
  showProject(project: string) {
    this.projectFocus = { project, n: (this.projectFocus?.n ?? 0) + 1 };
    this.detail = null;
    this.view = 'projects';
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
      const res = await runCli({ ...withContext(e.input, e.scope && this.contextChoice(e.scope)), ...answers });
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
      this.liveContext = '';
      this.live = { ...from, id: nextId++, input: { args }, title: titleOf({ args }), scope: 'tasks' };
    }
  }

  /**
   * Print something into the console that is not a command's answer (a review's banner and messages). An empty
   * `title` prints no `$` line above it.
   */
  addEntry(title: string, result: CliResult): Entry {
    const e = $state<Entry>({
      id: nextId++,
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
      id: nextId++,
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
    const res = await this.#exec(e);
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
  #wake(e: Entry) {
    this.#waiting.get(e.id)?.();
    this.#waiting.delete(e.id);
  }

  /** Run (or re-run) the Tasks view's live table. */
  async runLive(input: CliInput) {
    this.liveKey = JSON.stringify(input.args ?? input.line);
    this.liveContext = this.contextChoice('tasks') ?? '';
    if (!this.live) {
      this.live = {
        id: nextId++,
        input,
        title: titleOf(input),
        result: null,
        loading: true,
        failure: null,
        at: Date.now(),
        scope: 'tasks',
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
        scope: which,
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
      e.declined = true;
      this.#wake(e);
      return;
    }
    const res = await this.#exec(e, { ...e.answers, confirmed: true });
    this.#wake(e);
    if (res?.wrote && this.live) void this.refresh(this.live);
  }

  /** Answer a per-task question on an entry with the keys that were ticked. */
  async answerItems(e: Entry, keys: string[]) {
    const r = e.result;
    if (r?.kind !== 'confirm') return;
    const more: Answers = r.ask === 'permission' ? { approved: keys } : { extras: keys };
    if (r.ask === 'permission' && !keys.length) e.declined = true;
    const res = await this.#exec(e, { ...e.answers, ...more });
    this.#wake(e);
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

  #ticking = false;

  /**
   * Tick or untick a checkbox in a string UDA. The engine changes the text (`tc_core::markdown`), and the task is told the
   * new one with an ordinary `modify <uda>:<text>`. One at a time, so a second click never works from the text the first
   * one has just changed.
   */
  async tickBox(from: Entry | null, uuid: string, uda: string, text: string, line: number) {
    if (this.#ticking) return;
    const next = toggled(text, line);
    if (next === null) return;
    this.#ticking = true;
    try {
      await this.act(from, [uuid, 'modify', `${uda}:${next}`], {}, true);
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
    // A line that waits for an answer on an entry that is gone ends there.
    this.#aborted = true;
    for (const wake of this.#waiting.values()) wake();
    this.#waiting.clear();
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

  /** Put `text` at the prompt, with the cursor after it, ready to be carried on. */
  requestPrompt(text: string) {
    this.promptRequest = { text, n: (this.promptRequest?.n ?? 0) + 1 };
  }

  openDetail(uuid: string, from: Entry | null = null) {
    this.detail = { uuid, from };
  }

  /**
   * Show what the address says: a task's address opens its drawer, any other closes it and goes to the page it names. Returns
   * the address to put in the bar when it was written another way (a task found by a longer prefix) or names something that is
   * not there (`/`), and `undefined` otherwise. The pages that need the reports or the boards say nothing until those have
   * been read; the start asks again when they have.
   */
  async openFromAddress(path: string): Promise<string | undefined> {
    const place = parsePath(path);
    if (place?.page !== 'task') {
      this.detail = null;
      return this.openPage(place);
    }
    const prefix = place.prefix;
    // Already showing it (going back to a task that is open): nothing to look up.
    if (
      this.detail &&
      (this.detail.uuid.startsWith(prefix) || this.detail.uuid.replaceAll('-', '').startsWith(prefix))
    ) {
      return taskPath(this.detail.uuid);
    }
    let found: string[] = [];
    try {
      const r = (await runCli({ args: [prefixFilter(prefix), 'uuids'] })).result;
      found = r.kind === 'text' ? r.lines.join(' ').split(/\s+/).filter(Boolean) : [];
    } catch (e) {
      this.notify(e instanceof Error ? e.message : String(e), 'err');
    }
    if (found.length !== 1) {
      this.detail = null;
      this.notify(found.length ? `More than one task starts with ${prefix}.` : `No task starts with ${prefix}.`, 'err');
      return '/';
    }
    this.detail = { uuid: found[0], from: null };
    return taskPath(found[0]);
  }

  /** Go to a page by its place (not a task's): `null` is the front page, which is the home page. */
  private openPage(place: Place | null): string | undefined {
    if (!place) {
      // Which home page needs the reports and the boards.
      if (this.config && this.appSettings) this.goHome(true);
      return undefined;
    }
    switch (place.page) {
      case 'tasks':
        if (!this.config) return undefined;
        if (!this.reports.some((r) => r.name === place.report)) {
          this.notify(`There is no report called ${place.report}.`, 'err');
          this.goHome(true);
          return '/';
        }
        this.report = place.report;
        this.view = 'tasks';
        return placePath(place);
      case 'boards':
        if (!this.appSettings) return undefined;
        if (place.board !== null && !this.boards.some((b) => b.id === place.board)) {
          this.notify(`There is no board called ${place.board}.`, 'err');
          this.goHome(true);
          return '/';
        }
        this.boardId = place.board;
        this.view = 'boards';
        return place.board === null ? undefined : placePath(place);
      case 'settings':
        if (place.tab) this.settingsTab = place.tab;
        this.view = 'settings';
        return undefined;
      case 'task':
        return undefined;
      default:
        this.view = place.page;
        return undefined;
    }
  }
}

export const store = new Store();
