import {
  deleteDeviceSettings,
  getAppSettings,
  getConfig,
  getDeviceSettings,
  listDevices,
  putAppSettings,
  putDeviceSettings,
  runCli,
  useConfig,
} from '../api';
import { splitWords } from '../cmdline';
import { DEVICE_ID_KEY, deviceFile, mergeDevice } from '../deviceSettings';
import { aliasCommands } from '../taskCommands';
import { reminderSettings } from '../lockedReminders';
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
  serializeKanban,
  serializeSettings,
  type TaskCommand,
  type Transformation,
} from '../appsettings';
import { emptyKanbanSettings, resolveKanban, type Kanban, type KanbanSettings } from '../kanban';
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

function readDeviceId(): string | null {
  try {
    return localStorage.getItem(DEVICE_ID_KEY);
  } catch {
    return null;
  }
}

function writeDeviceId(id: string | null) {
  try {
    if (id) localStorage.setItem(DEVICE_ID_KEY, id);
    else localStorage.removeItem(DEVICE_ID_KEY);
  } catch {
    // Not remembered: this tab uses it until it is closed.
  }
}

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
  /** Which device's own settings this browser uses (spec 0027); null before it has any. */
  deviceId = $state<string | null>(readDeviceId());
  /** This device's own settings, read (null: none, or not read yet). */
  deviceRaw = $state<Record<string, unknown> | null>(null);
  /** This device lets AI agents in its tab use the app's tools (`web.agents` in its own file; spec 0031). */
  agentsOn = $derived.by(() => {
    const web = this.deviceRaw?.web;
    return !!web && typeof web === 'object' && (web as Record<string, unknown>).agents === true;
  });

  /** Turn the AI agents' tools on or off for this device, in its own file. Throws what the server refused. */
  async setAgents(on: boolean) {
    const raw = this.deviceRaw ?? {};
    const web = raw.web && typeof raw.web === 'object' ? { ...(raw.web as Record<string, unknown>) } : {};
    if (on) web.agents = true;
    else delete web.agents;
    await this.saveDevice({ ...raw, web });
  }
  /** What this device uses: `settings.json` with its own settings laid over it. */
  effectiveSettings = $derived<ParsedSettings | null>(
    this.deviceRaw && this.appSettings
      ? parseSettings(JSON.stringify(mergeDevice(this.appSettings.raw, this.deviceRaw)))
      : this.appSettings,
  );
  /** The default keys (`/keymaps/default.json`), once fetched. */
  keymapDefaults = $state<Record<string, unknown> | null>(null);
  /** The keys: the defaults, then `keys` and `web.keys` of `settings.json`, then this device's own. */
  keymap = $derived<Keymap>(resolveKeymap(this.effectiveSettings?.raw ?? {}, this.keymapDefaults));
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

  /** The kanbans in `settings.json` (spec 0026), and whether they are on. */
  kanbanSettings = $derived<KanbanSettings>(this.appSettings?.kanban ?? emptyKanbanSettings());
  /** Each kanban as it is drawn: its own settings, else Kanban's. */
  kanbans = $derived<Kanban[]>(this.kanbanSettings.kanbans.map((k) => resolveKanban(this.kanbanSettings, k)));
  /** The filter typed above a kanban, by its id, in place of its own while the page is open (nothing saved). */
  kanbanFilters = $state<Record<string, string>>({});
  /** The same for its Completed since. */
  kanbanSince = $state<Record<string, string>>({});

  /** The task commands in `settings.json`: words that stand for a command line, and buttons that run it. */
  taskCommands = $derived<TaskCommand[]>(this.appSettings?.commands ?? []);
  /** What a task's menus offer to run on it: the task commands, then the taskrc's own aliases. */
  menuCommands = $derived<TaskCommand[]>([
    ...this.taskCommands,
    ...aliasCommands(this.config?.config.settings ?? {}, this.taskCommands, this.appSettings?.aliasIcons),
  ]);
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
  homepage = $derived<Homepage | null>(this.effectiveSettings?.homepage ?? null);
  /** How the app draws things, from `settings.json`: the numeric UDAs shown as progress bars, by name. */
  appearance = $derived<Appearance>(this.appSettings?.appearance ?? emptyAppearance());
  /** For each transformation's condition, the UUIDs of the tasks it matches, as last asked (`resolveConditions`). */
  conditionTasks = $state<Record<string, string[]>>({});
  /** What the engine said no to in a condition, one line each. */
  conditionErrors = $state<string[]>([]);
  /**
   * The transformations as the cells and Markdown helpers take them (spec 0030): each with a condition carries the tasks
   * it matches, so it runs only on those; one not asked yet runs on none.
   */
  rules = $derived<Transformation[]>(
    this.appearance.transformations.map((r) =>
      r.condition ? { ...r, tasks: this.conditionTasks[r.condition] ?? [] } : r,
    ),
  );

  /**
   * Ask which tasks each transformation's condition matches: `( <condition> ) _uuids`, once per condition, which counts
   * every status and no context, as `task` does. After the settings are read and after every change.
   */
  async resolveConditions() {
    const conditions = [...new Set(this.appearance.transformations.flatMap((r) => (r.condition ? [r.condition] : [])))];
    const tasks: Record<string, string[]> = {};
    const errors: string[] = [];
    for (const c of conditions) {
      tasks[c] = [];
      try {
        const r = (await runCli({ args: ['rc.verbose=nothing', '(', ...splitWords(c), ')', '_uuids'] })).result;
        if (r.kind === 'text') tasks[c] = r.lines.map((l) => l.trim()).filter(Boolean);
        else if (r.kind === 'error') errors.push(`condition ${c}: ${r.message}`);
      } catch (e) {
        errors.push(`condition ${c}: ${e instanceof Error ? e.message : String(e)}`);
      }
    }
    this.conditionTasks = tasks;
    this.conditionErrors = errors;
  }

  /** Something was written: a task may have moved in or out of a condition. */
  override written() {
    super.written();
    void this.resolveConditions();
  }

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
    lucide.keep(this.menuCommands.flatMap((c) => (c.icon ? [c.icon] : [])));
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
    await this.loadDevice();
    await this.resolveConditions();
  }

  /** Read this device's own settings again (none: everyone's alone). A failure leaves what was there. */
  async loadDevice() {
    const id = this.deviceId;
    if (!id) {
      this.deviceRaw = null;
      return;
    }
    try {
      let raw = await getDeviceSettings(id);
      const migrated = raw && this.keymapDefaults && migrateSettings(raw, this.keymapDefaults);
      if (migrated) raw = migrated;
      if (this.deviceId === id) this.deviceRaw = raw;
    } catch {
      // Kept as it was: the next read tries again.
    }
  }

  /** The devices that have settings of their own. */
  listDevices(): Promise<string[]> {
    return listDevices();
  }

  /** Use the settings of device `id` from now on (`null`: none, everyone's alone). Nothing is written. */
  async useDevice(id: string | null) {
    this.deviceId = id;
    writeDeviceId(id);
    this.deviceRaw = null;
    await this.loadDevice();
  }

  /**
   * Save this device's own settings as `device` under `id` (this device's ID when left out). A new ID that was another's
   * renames it; the first save carries the reminders this browser had into the file. Throws what the server refused.
   */
  async saveDevice(device: Record<string, unknown>, id: string | null = this.deviceId) {
    if (!id) throw new Error('Give this device an ID first.');
    const first = !this.deviceRaw;
    const file = deviceFile(device, id, this.appSettings?.raw.version, first ? reminderSettings() : null);
    const text = JSON.stringify(file, null, 2) + '\n';
    fits(text);
    const from = this.deviceRaw && this.deviceId && this.deviceId !== id ? this.deviceId : undefined;
    await putDeviceSettings(id, text, from);
    this.deviceId = id;
    writeDeviceId(id);
    this.deviceRaw = file;
  }

  /** Save the device's file as written (the text editor), its ID kept. Throws what the server refused. */
  async saveDeviceText(text: string) {
    const id = this.deviceId;
    if (!id) throw new Error('Give this device an ID first.');
    fits(text);
    await putDeviceSettings(id, text);
    await this.loadDevice();
  }

  /** Take device `id`'s settings away; this device uses everyone's again when it was this one's. */
  async deleteDevice(id: string) {
    await deleteDeviceSettings(id);
    if (this.deviceId === id) await this.useDevice(null);
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

  /**
   * Save the kanbans (the rest of `settings.json` as it was), and the task commands too when given (the automatic setup's
   * moves), in one write; then read them back. Throws what the server refused.
   */
  async saveKanban(kanban: KanbanSettings, commands?: TaskCommand[]) {
    const raw = this.appSettings?.raw ?? {};
    const base = commands ? (JSON.parse(serializeSettings(raw, commands)) as Record<string, unknown>) : raw;
    const text = serializeKanban(base, kanban);
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
