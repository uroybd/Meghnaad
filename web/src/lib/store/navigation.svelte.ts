import type { FormSeed } from '../addPreview';
import { runCli } from '../api';
import { reportArgs, shellQuote } from '../cmdline';
import { parsePath, placePath, prefixFilter, taskPath, type Place, type SettingsTab } from '../route';
import type { Row } from '../types';
import { nextId, titleOf, type Entry } from './base.svelte';
import { SettingsStore } from './settings.svelte';

/** Where the app is: the page, what each page is filtered by, the drawers and dialogs open, and the address. */
export abstract class NavigationStore extends SettingsStore {
  /** The tag the Tags page should show (`n` changes on every request, so asking for the same tag again still reacts). */
  tagFocus = $state<{ tag: string; n: number } | null>(null);
  /** The same for a project on the Projects page. */
  projectFocus = $state<{ project: string; n: number } | null>(null);
  /** The filter on the Projects and Tags pages. They start as the pending tasks; taking that away brings in finished ones. */
  projectsFilter = $state('status.not:completed status.not:deleted status.not:recurring');
  tagsFilter = $state('status.not:completed status.not:deleted status.not:recurring');
  /** The filter on each of the Summary, Calendar and Burndown pages (Taskwarrior filter syntax). */
  summaryFilter = $state('');
  calendarFilter = $state('');
  burndownFilter = $state('');
  burndownPeriod = $state<'daily' | 'weekly' | 'monthly' | 'annual'>('daily');
  editing = $state<{ row: Row; from: Entry | null } | null>(null);
  /**
   * The add-task dialog (null = closed): `quick` is the one line with a preview of the task, otherwise the full form, which
   * starts from `seed` when the line was broken down first. `n` re-mounts it.
   */
  adding = $state<{ n: number; quick: boolean; line: string; seed?: FormSeed } | null>(null);
  notifyOpen = $state(false);
  /** The list of every key (`app.help`, `?`) is open. */
  keysHelpOpen = $state(false);

  /** Where the app is, as an address names it (`placePath`): the task in the drawer if one is open, else the page. */
  place = $derived<Place>(
    this.detail
      ? { page: 'task', prefix: this.detail.uuid.slice(0, 8) }
      : this.view === 'tasks'
        ? { page: 'tasks', report: this.report }
        : this.view === 'boards'
          ? { page: 'boards', board: this.boardId }
          : this.view === 'kanban'
            ? this.kanbanReport && this.kanbanId
              ? { page: 'kanban', kanban: this.kanbanId, report: this.kanbanReport }
              : { page: 'kanban', kanban: this.kanbanId }
            : this.view === 'settings'
              ? this.settingsTab === 'taskrc' && this.settingsFile
                ? { page: 'settings', tab: 'taskrc', file: this.settingsFile }
                : { page: 'settings', tab: this.settingsTab }
              : { page: this.view },
  );

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
      this.live = { ...from, id: nextId(), input: { args }, title: titleOf({ args }), scope: 'tasks' };
    }
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
      case 'kanban':
        if (!this.appSettings) return undefined;
        if (!this.kanbanSettings.enabled) {
          this.notify('Kanban is off. Set it up in Settings → Kanban.', 'err');
          this.openSettings('kanban');
          return placePath({ page: 'settings', tab: 'kanban' });
        }
        if (place.kanban !== null && !this.kanbans.some((k) => k.id === place.kanban)) {
          this.notify(`There is no kanban called ${place.kanban}.`, 'err');
          this.goHome(true);
          return '/';
        }
        this.kanbanId = place.kanban;
        this.kanbanReport = place.report ?? null;
        this.view = 'kanban';
        return place.kanban === null ? undefined : placePath(place);
      case 'settings':
        if (place.tab) {
          this.settingsTab = place.tab;
          this.settingsFile = place.file ?? null;
        }
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
