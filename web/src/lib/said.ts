// What a command printed besides its result, and a question with what `task` prints before it, as the pages show them
// outside the Console: in a toast, and in the box that asks.

import type { ConfirmItem, ConfirmResult, HookLine } from './types';

/** One question, with what `task` prints before it (`dependency.reminder`: the tasks it waits on, and that wait on it). */
export function questionText(item: ConfirmItem): string {
  return [...(item.note ?? []), item.question].join('\n');
}

/** What a box asks for a whole confirmation: the one question, or the heading and each task's lines. */
export function confirmText(r: ConfirmResult): string {
  if (r.items.length === 1) return questionText(r.items[0]);
  return [r.message, ...r.items.flatMap((i) => i.note ?? [])].join('\n');
}

/** The lines a command printed (its reminders, then what the hooks said), a line each. */
export function saidText(lines: HookLine[]): string {
  return lines.map((l) => l.text.trim()).join('\n');
}

/** A toast's style for them: a warning makes it one. */
export function saidKind(lines: HookLine[] | undefined): 'ok' | 'err' {
  return lines?.some((l) => l.kind === 'warn') ? 'err' : 'ok';
}
