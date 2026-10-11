// What tasksh's `review` needs in the taskrc: a `reviewed` date UDA and a `_reviewed` report that lists the tasks
// due for review. tasksh offers to write both the first time it runs; the same lines are written from here.
// (src/review.cpp in https://github.com/GothenburgBitFactory/taskshell)

import { splitWords } from './cmdline';
import type { ConfigResponse } from './types';

export const REVIEW_REPORT = '_reviewed';
export const REVIEW_DESCRIPTION = 'Tasksh review report.  Adjust the filter to your needs.';
export const REVIEW_SORT = 'reviewed+,modified+';
export const REVIEW_FILTER = '( reviewed.none: or reviewed.before:now-6days ) and ( +PENDING or +WAITING )';

/** `ok`: as tasksh defines it. `different`: there, but not the way tasksh needs it. */
export type Part = 'ok' | 'missing' | 'different';

export interface TaskshStatus {
  uda: Part;
  /** The `reviewed` UDA's type, when it is defined. */
  udaType: string | null;
  report: Part;
  /** The columns the `_reviewed` report has now (tasksh reads the uuid of each row). */
  columns: string[];
  /** The saved `_reviewed` filter and sort, when the report is defined. */
  filter: string | null;
  sort: string | null;
  /** Both parts are as tasksh needs them: `review` can run. */
  enabled: boolean;
}

export function taskshStatus(config: ConfigResponse | null): TaskshStatus {
  const uda = config?.config.udas?.reviewed;
  const report = config?.reports.find((r) => r.name === REVIEW_REPORT);
  const columns = report?.columns.map((c) => c.spec) ?? [];
  const udaPart: Part = !uda ? 'missing' : uda.type === 'date' ? 'ok' : 'different';
  const reportPart: Part = !report ? 'missing' : columns.length === 1 && columns[0] === 'uuid' ? 'ok' : 'different';
  return {
    uda: udaPart,
    udaType: uda?.type ?? null,
    report: reportPart,
    columns,
    filter: report?.filter ?? null,
    sort: report?.sort ?? null,
    enabled: udaPart === 'ok' && reportPart === 'ok',
  };
}

/** The part of the report you can adjust: which tasks are due, and in what order. */
export interface ReviewWanted {
  filter: string;
  sort: string;
}

/**
 * The `config` lines (name, value) that bring the taskrc to what is wanted, leaving alone what is already so.
 * `columns` goes before the rest of the report: a report without columns is not kept, so a description sent first
 * would be lost.
 */
export function setupLines(config: ConfigResponse | null, want: ReviewWanted): [string, string][] {
  const s = taskshStatus(config);
  const lines: [string, string][] = [];
  if (s.uda !== 'ok') lines.push(['uda.reviewed.type', 'date']);
  if (s.uda === 'missing' || !config?.config.udas?.reviewed?.label) lines.push(['uda.reviewed.label', 'Reviewed']);
  if (s.report !== 'ok') lines.push([`report.${REVIEW_REPORT}.columns`, 'uuid']);
  if (s.report === 'missing') lines.push([`report.${REVIEW_REPORT}.description`, REVIEW_DESCRIPTION]);
  if (s.report !== 'ok' || s.sort !== want.sort) lines.push([`report.${REVIEW_REPORT}.sort`, want.sort]);
  if (s.report !== 'ok' || s.filter !== want.filter) lines.push([`report.${REVIEW_REPORT}.filter`, want.filter]);
  return lines;
}

/** One `config` line as the arguments to send: no questions asked, as tasksh runs it. */
export function configArgs([name, value]: [string, string]): string[] {
  return ['rc.confirmation:no', 'rc.verbose:nothing', 'config', name, value];
}

/**
 * `review` or `review 10` (to stop after ten), as tasksh reads it: the word may be cut to three letters or more, and
 * `task` may come before it. Anything else is not the review command and goes to Taskwarrior as it was typed.
 */
export function parseReview(line: string): { limit: number } | null {
  const words = splitWords(line.trim());
  if (words[0] === 'task') words.shift();
  const [cmd, count, ...more] = words;
  if (!cmd || cmd.length < 3 || !'review'.startsWith(cmd.toLowerCase()) || more.length) return null;
  if (count === undefined) return { limit: 0 };
  return /^\d+$/.test(count) ? { limit: Number(count) } : null;
}

export type ReviewKey = 'reviewed' | 'skip' | 'edit' | 'modify' | 'complete' | 'delete' | 'quit';

/** What an answer at the review menu means: Enter or `r` mark it reviewed; null is a word the menu does not know. */
export function menuKey(input: string): ReviewKey | null {
  switch (input.trim().toLowerCase()) {
    case '':
    case 'r':
      return 'reviewed';
    case 's':
      return 'skip';
    case 'e':
      return 'edit';
    case 'm':
      return 'modify';
    case 'c':
      return 'complete';
    case 'd':
      return 'delete';
    case 'q':
      return 'quit';
    default:
      return null;
  }
}

// tasksh's own words (src/review.cpp), so a session reads the same here.
export const REVIEW_MENU = '(Enter) Mark as reviewed, (s)kip, (e)dit, (m)odify, (c)omplete, (d)elete, (q)uit';
export const REVIEW_MODIFY_PROMPT = 'Enter modification args [example: +tag -tag /teh/the/ project:X]';
export const REVIEW_NOTHING = 'There are no tasks needing review.';
export const REVIEW_INTRO =
  'The review process is important for keeping your list accurate, so you are working on the right tasks. ' +
  "For each task you are shown, look at the metadata. Determine whether the task needs to be changed (enter 'e' to " +
  "edit), or whether it is accurate ('enter' or 'r' to mark as reviewed). You may skip a task ('s') but a skipped " +
  'task is not considered reviewed. You may stop at any time, and resume later right where you left off.';
export const reviewEnd = (reviewed: number, total: number) =>
  `End of review. ${reviewed} out of ${total} tasks reviewed.`;
export const notRecognized = (word: string) => `Command '${word}' is not recognized.`;

/** The question before defining one part, in tasksh's words (and what is there now, where tasksh would say nothing). */
export function setupQuestion(part: 'uda' | 'report', s: TaskshStatus): string {
  if (part === 'uda') {
    return s.uda === 'missing'
      ? "Tasksh needs to define a 'reviewed' UDA of type 'date' for all tasks.  Ok to proceed?"
      : `The 'reviewed' UDA is of type '${s.udaType}' now. Tasksh needs it to be of type 'date'.  Ok to change it?`;
  }
  return s.report === 'missing'
    ? "Tasksh needs to define a '_reviewed' report to identify tasks needing review.  Ok to proceed?"
    : `The '_reviewed' report has the columns '${s.columns.join(', ')}' now. Tasksh needs it to have only 'uuid'.  Ok to replace them?`;
}

/** The `config` lines for one part of the setup. */
export function partLines(lines: [string, string][], part: 'uda' | 'report'): [string, string][] {
  const prefix = part === 'uda' ? 'uda.reviewed.' : `report.${REVIEW_REPORT}.`;
  return lines.filter(([name]) => name.startsWith(prefix));
}
