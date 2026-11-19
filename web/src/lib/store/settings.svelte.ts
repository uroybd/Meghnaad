import { getAppSettings, getConfig, putAppSettings, useConfig } from '../api';
import {
  emptyAppearance,
  MAX_SETTINGS_BYTES,
  parseSettings,
  serializeAppearance,
  serializeBoards,
  serializeHomepage,
  type Appearance,
  type Homepage,
  type ParsedSettings,
  type TaskCommand,
} from '../appsettings';
import type { Board, ControlValue } from '../boards';
import type { Vocab } from '../completion';
import {
  fetchCommandsDoc,
  fetchKeymapDefaults,
  migrateSettings,
  resolveKeymap,
  type CommandsDoc,
  type Keymap,
} from '../keymap';
import { lucide } from '../lucideLib.svelte';
import { loadChoices, NO_CONTEXT, saveChoices, type Scope } from '../pageContext';
import { isHiddenReport, offeredReports as pickable } from '../reportList';
import { settingsData } from '../settingsData.svelte';
import type { ConfigResponse, ReportMeta } from '../types';
import { StoreBase } from './base.svelte';

const SHOW_HIDDEN_KEY = 'tw-web-show-hidden-reports';

function readFlag(key: string): boolean {
  try {
    return localStorage.getItem(key) === '1';
  } catch {
    return false;
  }
}

/** Throw when `text` would not fit in `settings.json`. */
function fits(text: string) {
  if (new Blob([text]).size > MAX_SETTINGS_BYTES)
    throw new Error(`The settings would be larger than ${MAX_SETTINGS_BYTES / 1024} KB.`);
}

/** The settings the engine runs on and the app's own `settings.json`, and what is read from them. */
export abstract class SettingsStore extends StoreBase {
  config = $state<ConfigResponse | null>(null);
  configError = $state<string | null>(null);
  /** The app's own `settings.json`, read (null until it has been). */
  appSettings = $state<ParsedSettings | null>(null);
  /** The default keys (`/keymaps/default.json`), once fetched. */
  keymapDefaults = $state<Record<string, unknown> | null>(null);
  /** The keys: the defaults, then `keys` and `web.keys` of `settings.json`. */
  keymap = $derived<Keymap>(resolveKeymap(this.appSettings?.raw ?? {}, this.keymapDefaults));
  /** The commands of the `:` line as people read them (`/keymaps/commands.json`), once fetched. */
  commandsDoc = $state<CommandsDoc | null>(null);
  /**
   * The filter typed above a board to look at it another way, by the board's id: it takes the place of the board's own filter
   * while the page is open (nothing saved). A board not here shows its own.
   */
  boardFilters = $state<Record<string, string>>({});
  /**
   * The values set for a board's controls (by board, then by control name) in place of the board's own, while the page
   * is open (nothing saved). A control not here has its own.
   */
  boardControls = $state<Record<string, Record<string, ControlValue>>>({});

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
  /** The context each page chose (a name, or `none`); a page not here follows the active one. */
  contextChoices = $state<Record<string, string>>(typeof localStorage === 'undefined' ? {} : loadChoices());
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
  /** Where the app opens: a report or a board of `settings.json`, or `next` when it says nothing. */
  homepage = $derived<Homepage | null>(this.appSettings?.homepage ?? null);
  /** How the app draws things, from `settings.json`: the numeric UDAs shown as progress bars, by name. */
  appearance = $derived<Appearance>(this.appSettings?.appearance ?? emptyAppearance());

  /** Fetch the default keys and the commands. Without them the page has no keys, only its buttons, so a failure is not said. */
  async loadKeymapDefaults() {
    try {
      this.keymapDefaults = await fetchKeymapDefaults();
    } catch {
      // Left as none: the keys are a convenience.
    }
    try {
      this.commandsDoc ??= await fetchCommandsDoc();
    } catch {
      // Left as none: the lists of keys show no commands.
    }
  }

  setShowHiddenReports(on: boolean) {
    this.showHiddenReports = on;
    try {
      localStorage.setItem(SHOW_HIDDEN_KEY, on ? '1' : '0');
    } catch {
      /* private mode: the choice lasts until the page is closed */
    }
  }

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

  /**
   * The saved settings changed (the taskrc dialog, `config`): read them again, and draw again what is on screen,
   * since they can change what it shows (colours, a UDA, a report, `dateformat`). The settings screens read their files
   * again when next shown, except `fresh`: files the caller has just saved and already holds (spec 0023).
   */
  async settingsChanged(fresh: string[] = []) {
    settingsData.changed(fresh);
    await this.loadConfig();
    if (this.live) void this.refresh(this.live);
    this.rev++;
  }

  /** The icons the task commands use are kept in the browser, and only those. */
  rememberIcons() {
    lucide.keep(this.taskCommands.flatMap((c) => (c.icon ? [c.icon] : [])));
  }

  /**
   * Read `settings.json`, brought to the latest version and saved so when it was older (the keys' migration reads the
   * default keys, so they are fetched first). A failure leaves what was there: the commands are a convenience, not
   * needed to work.
   */
  async loadAppSettings() {
    try {
      let raw = (await getAppSettings()) as Record<string, unknown>;
      if (raw && typeof raw === 'object' && !Array.isArray(raw)) {
        if (!this.keymapDefaults) await this.loadKeymapDefaults();
        // Without the default keys the old keys cannot be read rightly; the file waits for next time.
        const migrated = this.keymapDefaults && migrateSettings(raw, this.keymapDefaults);
        if (migrated) {
          raw = migrated;
          // Not saved is not lost: the old file is brought up to date the same way next time.
          await putAppSettings(JSON.stringify(migrated, null, 2) + '\n').catch(() => undefined);
        }
      }
      this.appSettings = parseSettings(JSON.stringify(raw));
      this.rememberIcons();
    } catch {
      this.appSettings ??= parseSettings('');
    }
  }

  /** Save how the app draws things, the rest of `settings.json` as it was. Throws what the server refused. */
  async saveAppearance(appearance: Appearance) {
    await putAppSettings(serializeAppearance(this.appSettings?.raw ?? {}, appearance));
    await this.loadAppSettings();
  }

  /** Save the whole of `settings.json` as the Keybindings tab edited it. Throws what the server refused. */
  async saveKeys(settings: Record<string, unknown>) {
    const text = JSON.stringify(settings, null, 2) + '\n';
    fits(text);
    await putAppSettings(text);
    await this.loadAppSettings();
  }

  /** Save where the app opens (`null`: `next`), the rest of `settings.json` as it was. Throws what the server refused. */
  async saveHomepage(homepage: Homepage | null) {
    await putAppSettings(serializeHomepage(this.appSettings?.raw ?? {}, homepage));
    await this.loadAppSettings();
  }

  /** Save the boards (the rest of `settings.json` as it was) and read them back. Throws what the server refused. */
  async saveBoards(boards: Board[], homepage?: Homepage | null) {
    const text = serializeBoards(this.appSettings?.raw ?? {}, boards, homepage);
    fits(text);
    await putAppSettings(text);
    await this.loadAppSettings();
  }

  /** Settings a save answered with, in place of reading them again. */
  async takeConfig(config: ConfigResponse) {
    await useConfig(config);
    this.config = config;
    this.configError = null;
  }

  async loadConfig(reuse = false) {
    try {
      this.config = await getConfig(reuse);
      this.configError = null;
      void this.refreshNames();
    } catch (e) {
      this.configError = e instanceof Error ? e.message : String(e);
    }
  }

  /** The taskrc's `confirmation` (on unless turned off): the delete buttons ask before they delete. */
  get confirmation(): boolean {
    const v = this.config?.config.settings?.confirmation;
    return v === undefined || /^(1|y|yes|on|true)$/i.test(v.trim());
  }
}
