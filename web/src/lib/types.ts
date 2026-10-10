// Mirrors the JSON produced by tc-core (see crates/tc-core/src/{run,cli,model}.rs).

export interface Note {
  entry: number;
  text: string;
}

export interface Facts {
  uuid: string;
  /** pending | completed | deleted | recurring */
  status: string;
  description: string;
  project: string | null;
  priority: string | null;
  tags: string[];
  annotations: Note[];
  entry: number | null;
  modified: number | null;
  start: number | null;
  end: number | null;
  due: number | null;
  wait: number | null;
  scheduled: number | null;
  until: number | null;
  depends: string[];
  blocked: boolean;
  blocking: boolean;
  recur: string | null;
  parent: string | null;
  /** On a recurring template: one letter per instance (`+` done, `-` pending, `X` deleted, `W` waiting). */
  mask: string | null;
  /** On an instance: its position in the parent's mask. */
  imask: number | null;
  /** Non-core properties: UDA values, and orphans (see `Row.orphans`). */
  extra: Record<string, string>;
}

/** A colour style from the engine: palette indexes (0-15 basic and bright, 16-231 cube, 232-255 grays). */
export interface Resolved {
  bold?: boolean;
  underline?: boolean;
  inverse?: boolean;
  fg?: number;
  bg?: number;
}

/** A run of coloured text. */
export interface Span {
  text: string;
  style?: Resolved;
}

export interface Row extends Facts {
  /** How the colour rules (`color.*`) colour this task; absent when none applies or colour is off. */
  style?: Resolved;
  urgency: number;
  /** Working-set number; specific to the web UI and usually differs from a desktop's. */
  id: number | null;
  /** Every virtual tag that applies: only `info` fills it. */
  virtual_tags?: string[];
  /** The WAITING virtual tag (absent when false): what a table reads. */
  waiting?: boolean;
  /** Properties in `extra` that the taskrc doesn't define as UDAs: shown, never edited. */
  orphans: string[];
  /** Tracked time in seconds when `journal.time` is on (includes the current stretch). */
  active_seconds: number | null;
  /** Tracked work sessions (only when `journal.time` is on); the running one has no `end`. */
  sessions: Session[];
  /** How many of the tasks this one depends on are still open (absent when none). */
  pending_deps?: number;
}

export type HistoryKind =
  | 'set'
  | 'changed'
  | 'deleted'
  | 'note_added'
  | 'note_changed'
  | 'note_deleted'
  | 'tag_added'
  | 'tag_deleted'
  | 'dep_added'
  | 'dep_deleted';

export interface HistoryChange {
  kind: HistoryKind;
  /** The property; for the tag and dependency kinds, the tag or the uuid. */
  prop: string;
  old: string | null;
  value: string | null;
  /** The values are epoch seconds to show in the date format. */
  date: boolean;
  /** For a deleted `start`: how long the task ran, already formatted. */
  duration: string | null;
}

/** The changes made at one moment. */
export interface HistoryEntry {
  at: number;
  changes: HistoryChange[];
}

export interface Session {
  start: number;
  end: number | null;
  seconds: number;
}

export type ColumnKind =
  | 'id'
  | 'string'
  | 'description'
  | 'project'
  | 'priority'
  | 'status'
  | 'tags'
  | 'date'
  | 'number'
  | 'duration'
  | 'uuids'
  | 'notes';

export interface Column {
  spec: string;
  name: string;
  format: string | null;
  label: string;
  kind: ColumnKind;
}

export interface ReportResult {
  kind: 'report';
  report: string;
  description: string | null;
  columns: Column[];
  rows: Row[];
  breaks: boolean[];
  matched: number;
  /** The sort spec in effect (the report's, or an `rc.report.<name>.sort:` override). */
  sort: string | null;
}

export interface InfoResult {
  kind: 'info';
  tasks: Row[];
}
export interface SummaryRow {
  /** The full project name; empty for tasks without a project. */
  project: string;
  /** `(none)`, or the last part of the name. */
  label: string;
  /** How many levels down (`Home.Kitchen` is 1). */
  depth: number;
  remaining: number;
  completed: number;
  avg_age: string;
  complete: string;
  /** Filled cells of a 30-cell bar. */
  bar: number;
}
export interface SummaryResult {
  kind: 'summary';
  rows: SummaryRow[];
}

export type DueState = 'overdue' | 'due-today' | 'due';
export interface CalendarDay {
  day: number;
  today: boolean;
  weekend: boolean;
  holiday: boolean;
  scheduled: boolean;
  due: DueState | null;
}
export interface CalendarWeek {
  number: number | null;
  days: (CalendarDay | null)[];
}
export interface CalendarMonth {
  year: number;
  month: number;
  name: string;
  weeks: CalendarWeek[];
}
export interface CalendarResult {
  kind: 'calendar';
  months: CalendarMonth[];
  /** Two-letter column headings, from the first day of the week. */
  weekdays: string[];
  week_numbers: boolean;
  legend: boolean;
  due_colours: boolean;
  holiday_colours: boolean;
  holidays: { date: number; name: string }[] | null;
  details: ReportResult | null;
}

export interface BurndownBar {
  /** Start of the period, in epoch seconds. */
  epoch: number;
  major: string;
  minor: string;
  pending: number;
  started: number;
  done: number;
}
export interface BurndownResult {
  kind: 'burndown';
  period: 'daily' | 'weekly' | 'monthly' | 'annual';
  title: string;
  /** Oldest first. */
  bars: BurndownBar[];
  /** Tasks finished before the first bar; they are on every bar. */
  carryover_done: number;
  /** The y axis: 0, half, top. */
  y_labels: [number, number, number];
  net_fix_rate: number | null;
  completion: { epoch: number; in_secs: number; vague: string } | null;
  no_convergence: boolean;
  peak_count: number;
  peak_day: number;
  current_count: number;
}

export interface TableResult {
  kind: 'table';
  title: string | null;
  footer?: string[];
  /** Indexes of rows to emphasise (settings changed from their default). */ highlight?: number[];
  /** Columns of numbers, drawn aligned to the right. */
  right?: number[];
  headers: string[];
  rows: string[][];
}
export interface TextResult {
  kind: 'text';
  lines: string[];
}
/** Coloured text (`colors`, the history graph). */
export interface StyledResult {
  kind: 'styled';
  lines: Span[][];
  /** Draw the colours as they are (a palette), not softened to suit the page. */
  swatch?: boolean;
}
/** A file to offer as a download (`export`). */
export interface FileResult {
  kind: 'file';
  name: string;
  mime: string;
  text: string;
  /** How many tasks are in it. */
  count: number;
}
/** `import`: the page offers to pick a file; the file itself goes to `/api/import`. */
export interface ImportResult {
  kind: 'import';
}
/** What importing a file did (or, for a check, would do). */
export interface ImportReport {
  /** Whether the tasks were written. */
  applied: boolean;
  added: number;
  modified: number;
  skipped: number;
  lines: { action: 'add' | 'mod' | 'skip'; uuid: string; description: string }[];
  /** How many tasks there were beyond `lines`. */
  more: number;
  warnings: string[];
  feedback: { kind: 'info' | 'warn'; text: string }[];
}
export interface JsonResult {
  kind: 'json';
  value: unknown;
}
export interface ChangedResult {
  kind: 'changed';
  message: string;
  tasks: { uuid: string; id: number | null; description: string }[];
}
/** One question about one task. */
export interface ConfirmItem {
  /** What to send back to say yes. */
  key: string;
  uuid: string;
  id: number | null;
  description: string;
  question: string;
}
/**
 * Taskwarrior wants an answer: `plain` is one yes/no; `permission` has a question per task the
 * command would change (tick the ones to go ahead with); `extras` are follow-up questions that
 * only arise once those are answered (repair a dependency chain, change a recurring series).
 */
export interface ConfirmResult {
  kind: 'confirm';
  message: string;
  ask: 'plain' | 'permission' | 'extras';
  items: ConfirmItem[];
}
export interface ErrorResult {
  kind: 'error';
  message: string;
}

export type CliResult =
  | ReportResult
  | InfoResult
  | TableResult
  | SummaryResult
  | CalendarResult
  | BurndownResult
  | TextResult
  | JsonResult
  | FileResult
  | ImportResult
  | StyledResult
  | ChangedResult
  | ConfirmResult
  | ErrorResult;

/** How the server understood the command line. */
export interface CommandInfo {
  /** Canonical command or report name. */
  name: string;
  /** True when `name` is a report (so the UI should focus it). */
  report: boolean;
  /** Filter words without the command, e.g. ['project:Work', '+errand']. */
  filter: string[];
}

/** A line a hook printed (see `hooks.rs`). */
export interface HookLine {
  kind: 'info' | 'warn';
  text: string;
}

export interface CliResponse {
  wrote: boolean;
  /** What the hooks printed while running this command. */
  feedback?: HookLine[];
  result: CliResult;
  command: CommandInfo | null;
}

export type UdaType = 'string' | 'numeric' | 'date' | 'duration' | 'uuid';

export interface UdaDef {
  name: string;
  type: UdaType;
  label: string | null;
  values: string[];
  default: string | null;
  indicator: string | null;
}

export interface ReportMeta {
  name: string;
  description: string | null;
  columns: Column[];
  filter: string | null;
  sort: string | null;
}

export interface ConfigResponse {
  config: {
    udas: Record<string, UdaDef>;
    reports: Record<string, unknown>;
    contexts: Record<string, { name: string; read: string | null; write: string | null; rc: Record<string, string> }>;
    active_context: string | null;
    urgency: Record<string, number>;
    settings: Record<string, string>;
  };
  reports: ReportMeta[];
  /** The annotation texts `journal.time` writes on start/stop; null when journalling is off. */
  journal: { start: string; stop: string } | null;
  /** There are earlier saved settings to restore. */
  has_previous: boolean;
  /** Whether tasks are coloured (`color`). */
  color: boolean;
  /** Every colour in force, by name without `color.` (`calendar.today`): palette indexes, for the charts. */
  colors: Record<string, Resolved>;
  /** Taskwarrior's built-in urgency coefficients: what a setting falls back to. */
  urgency_defaults: Record<string, number>;
  /** `urgency.inherit`: blocking tasks take the highest urgency of what they block. */
  urgency_inherit: boolean;
  /** Saved settings exist but couldn't be read (they are left untouched). */
  config_error: string | null;
}

export interface TaskrcResponse {
  udas: number;
  reports: number;
  contexts: number;
  /** Setting *names* refused for safety (sync.*, credentials). Values are never sent back. */
  blocked: string[];
  ignored: string[];
  warnings: string[];
  /** The settings as now saved, rendered as a taskrc (nothing blocked is ever in it). */
  text: string;
}
