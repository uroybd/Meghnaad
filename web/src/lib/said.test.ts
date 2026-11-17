import { describe, expect, it } from 'vitest';
import { confirmText, questionText, saidKind, saidText } from './said';
import type { ConfirmItem, ConfirmResult } from './types';

const item = (note?: string[]): ConfirmItem => ({
  key: 'dep:u',
  uuid: 'u',
  id: 3,
  description: 'c',
  question: 'Would you like the dependency chain fixed?',
  note,
});
const REMINDER = ['Task 3 is blocked by:', '  1 a', '  2 b', 'and is blocking:', '  4 d'];

describe('what a command said, outside the Console', () => {
  it('puts the reminder before the question, as task prints it', () => {
    expect(questionText(item(REMINDER))).toBe(`${REMINDER.join('\n')}\nWould you like the dependency chain fixed?`);
    expect(questionText(item())).toBe('Would you like the dependency chain fixed?');
  });

  it('asks one question as it is, and several under their heading', () => {
    const one: ConfirmResult = { kind: 'confirm', message: 'x', ask: 'extras', items: [item(REMINDER)] };
    expect(confirmText(one)).toBe(questionText(item(REMINDER)));
    const two: ConfirmResult = {
      ...one,
      message: 'A few more questions about these tasks.',
      items: [item(['n']), item()],
    };
    expect(confirmText(two)).toBe('A few more questions about these tasks.\nn');
  });

  it('gives each line its own line, and is a warning when one is', () => {
    const lines = [
      { kind: 'warn' as const, text: 'Task 3 is blocked by:' },
      { kind: 'warn' as const, text: '  1 a' },
    ];
    expect(saidText(lines)).toBe('Task 3 is blocked by:\n1 a');
    expect(saidKind(lines)).toBe('err');
    expect(saidKind([{ kind: 'info', text: 'hi' }])).toBe('ok');
    expect(saidKind(undefined)).toBe('ok');
  });
});
