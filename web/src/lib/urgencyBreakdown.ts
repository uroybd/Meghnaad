// How a task's urgency adds up, as `task info` writes it: the engine's terms (each a label, the measure, the coefficient
// and what it adds, as text) and the total. The info panel draws them; nothing is worked out here.

import { callHelper } from '../engine/helpers';
import { tzOffsetSeconds } from './api';
import type { ConfigResponse, Row } from './types';

export interface UrgencyBreakdown {
  terms: [label: string, measure: string, coefficient: string, value: string][];
  total: string;
}

export function urgencyBreakdown(task: Row, config: ConfigResponse['config'] | undefined | null): UrgencyBreakdown {
  return callHelper<UrgencyBreakdown>(
    'urgency_breakdown',
    task,
    config ?? {},
    Math.floor(Date.now() / 1000),
    tzOffsetSeconds(),
  );
}
