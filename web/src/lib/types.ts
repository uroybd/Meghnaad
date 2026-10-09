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

export interface Row extends Facts {
  urgency: number;
  /** Working-set number; specific to the web UI and usually differs from a desktop's. */
  id: number | null;
  virtual_tags: string[];
  /** Properties in `extra` that the taskrc doesn't define as UDAs: shown, never edited. */
  orphans: string[];
  /** Tracked time in seconds when `journal.time` is on (includes the current stretch). */
  active_seconds: number | null;
  /** Tracked work sessions (only when `journal.time` is on); the running one has no `end`. */
  sessions: Session[];
}

export interface Session {
  start: number;
  end: number | null;
  seconds: number;
}

export type ColumnKind =
  | 'id' | 'string' | 'description' | 'project' | 'priority' | 'status' | 'tags'
  | 'date' | 'number' | 'duration' | 'uuids' | 'notes';

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

export interface InfoResult { kind: 'info'; tasks: Row[] }
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
export interface SummaryResult { kind: 'summary'; rows: SummaryRow[] }

export type DueState = 'overdue' | 'due-today' | 'due';
export interface CalendarDay {
  day: number;
  today: boolean;
  weekend: boolean;
  holiday: boolean;
  scheduled: boolean;
  due: DueState | null;
}
export interface CalendarWeek { number: number | null; days: (CalendarDay | null)[] }
export interface CalendarMonth { year: number; month: number; name: string; weeks: CalendarWeek[] }
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

export interface TableResult { kind: 'table'; title: string | null; headers: string[]; rows: string[][] }
export interface TextResult { kind: 'text'; lines: string[] }
export interface JsonResult { kind: 'json'; value: unknown }
export interface ChangedResult {
  kind: 'changed';
  message: string;
  tasks: { uuid: string; id: number | null; description: string }[];
}
export interface ConfirmResult {
  kind: 'confirm';
  message: string;
  count: number;
  /** The question is about the rest of a recurring series (answer with all / only this one). */
  recurrence?: boolean;
}
export interface ErrorResult { kind: 'error'; message: string }

export type CliResult =
  | ReportResult | InfoResult | TableResult | SummaryResult | CalendarResult | TextResult | JsonResult
  | ChangedResult | ConfirmResult | ErrorResult;

/** How the server understood the command line. */
export interface CommandInfo {
  /** Canonical command or report name. */
  name: string;
  /** True when `name` is a report (so the UI should focus it). */
  report: boolean;
  /** Filter words without the command, e.g. ['project:Work', '+errand']. */
  filter: string[];
}

export interface CliResponse {
  wrote: boolean;
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
