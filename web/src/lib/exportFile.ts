import { callHelper } from '../engine/helpers';
import type { ConfigResponse } from './types';

// The file the Export button over a table makes, from what `export` wrote: the engine's (`tc_core::table_export`),
// shared with the phone and the terminal app. The engine says which tasks; the person says how the file looks.

export type Format = 'json' | 'csv';
/** Every field the tasks have, the table's columns, or the ones picked. */
export type Fields = 'all' | 'table' | 'custom';

export interface ExportChoice {
  format: Format;
  /** JSON only: a list (`json.array=on`) or one object per line; null: as the taskrc says. */
  array: boolean | null;
  fields: Fields;
  /** The fields picked, for `custom`. */
  picked: string[];
}

/** Every field the tasks in what `export` wrote have, in the order they first appear. */
export const exportFields = (text: string): string[] => callHelper<string[]>('export_fields', text);

/** `json.array` as the taskrc has it, as Taskwarrior reads it (on unless it says otherwise). */
export const jsonArrayOf = (cfg: ConfigResponse['config'] | undefined): boolean =>
  callHelper<boolean>('json_array', cfg ?? {});

/** The fields a table shows, once each, in its order (`due.relative` is `due`). */
export const tableFields = (columns: string[]): string[] => callHelper<string[]>('table_fields', columns);

/** The file for `choice`: the engine's name with the format's extension, the text, its type. */
export function renderExport(
  exported: { name: string; text: string },
  choice: ExportChoice,
  columns: string[],
  cfg: ConfigResponse['config'] | undefined,
): { name: string; text: string; mime: string } {
  return callHelper('export_file', exported.name, exported.text, choice, columns, cfg ?? {});
}
