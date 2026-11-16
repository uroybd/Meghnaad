// The `remind_in` UDA: a task's own times before its due and scheduled dates (`10,1h`). Reminders work without it,
// from the setting; defining it lets `task add … remind_in:10,1h` keep the value and shows it on the task's page.

import type { ConfigResponse } from './types';
import type { Part } from './tasksh';

export const REMIND_UDA = 'remind_in';

/** `ok`: a string UDA. `different`: defined with another type. */
export function remindUdaStatus(config: ConfigResponse | null): Part {
  const uda = config?.config.udas?.[REMIND_UDA];
  return !uda ? 'missing' : uda.type === 'string' ? 'ok' : 'different';
}

/** The `config` lines (name, value) that define it; none when it is already so. The label stays when there is one. */
export function remindUdaLines(config: ConfigResponse | null): [string, string][] {
  const part = remindUdaStatus(config);
  if (part === 'ok') return [];
  const lines: [string, string][] = [[`uda.${REMIND_UDA}.type`, 'string']];
  if (!config?.config.udas?.[REMIND_UDA]?.label) lines.push([`uda.${REMIND_UDA}.label`, 'Remind in']);
  return lines;
}
