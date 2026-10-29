// What `priority` can be: Taskwarrior defines it as a UDA (`uda.priority.values`, `H,M,L,` unless the taskrc says
// otherwise), so a taskrc may rename the levels (`Critical,Important,`) or put the blank between two of them.

import type { UdaDef } from './types';

/** The values `priority` takes, highest first, the blank one left out. */
export function priorityValues(udas: Record<string, UdaDef> | undefined): string[] {
  const own = udas?.priority?.values.filter(Boolean) ?? [];
  return own.length ? own : ['H', 'M', 'L'];
}

/** The values as a sentence: `H, M or L`. */
export function orList(values: string[]): string {
  return values.length > 1 ? `${values.slice(0, -1).join(', ')} or ${values[values.length - 1]}` : (values[0] ?? '');
}
