import { describe, expect, it } from 'vitest';
import { describeChange } from './history';
import type { HistoryChange, HistoryKind } from './types';

const show = (n: number) => `<${n}>`;
const c = (kind: HistoryKind, prop: string, over: Partial<HistoryChange> = {}): HistoryChange => ({
  kind,
  prop,
  old: null,
  value: null,
  date: false,
  duration: null,
  ...over,
});
const say = (x: HistoryChange) => describeChange(x, show);

// The sentences below are the ones the real `task` 3.5.0 printed for the same changes.
describe('describeChange', () => {
  it('says what was set, with dates in the date format', () => {
    expect(say(c('set', 'description', { value: 'Alpha' }))).toBe("Description set to 'Alpha'.");
    expect(say(c('set', 'due', { value: '1760000000', date: true }))).toBe("Due set to '<1760000000>'.");
    expect(say(c('set', 'priority', { value: 'H' }))).toBe("Priority set to 'H'.");
  });

  it('says what a property changed from and to', () => {
    expect(say(c('changed', 'project', { old: 'home', value: 'work' }))).toBe("Project changed from 'home' to 'work'.");
    expect(say(c('changed', 'due', { old: '100', value: '200', date: true }))).toBe(
      "Due changed from '<100>' to '<200>'.",
    );
    expect(say(c('changed', 'status', { old: 'pending', value: 'completed' }))).toBe(
      "Status changed from 'pending' to 'completed'.",
    );
  });

  it('shows a date value that is not a number as it is, like Taskwarrior', () => {
    expect(say(c('set', 'due', { value: 'soon', date: true }))).toBe("Due set to 'soon'.");
    expect(say(c('set', 'due', { value: '0', date: true }))).toBe("Due set to '0'.");
  });

  it('says when a property was deleted, with how long a start had run', () => {
    expect(say(c('deleted', 'wait', { old: '5', date: true }))).toBe('Wait deleted.');
    expect(say(c('deleted', 'start', { old: '1', date: true, duration: '0:00:03' }))).toBe(
      'Start deleted (duration: 0:00:03).',
    );
    expect(say(c('deleted', 'start', { old: '1', date: true, duration: '-3:10:28' }))).toBe(
      'Start deleted (duration: -3:10:28).',
    );
    // No beginning in the history to measure from: say no more than is known.
    expect(say(c('deleted', 'start', { old: '1', date: true }))).toBe('Start deleted.');
  });

  it('names tags, annotations and dependencies', () => {
    expect(say(c('tag_added', 'y'))).toBe("Tag 'y' added.");
    expect(say(c('tag_deleted', 'x'))).toBe("Tag 'x' deleted.");
    expect(say(c('note_added', 'annotation_1', { value: 'a note' }))).toBe("Annotation of 'a note' added.");
    expect(say(c('note_changed', 'annotation_1', { old: 'a', value: 'b' }))).toBe("Annotation changed to 'b'.");
    expect(say(c('note_deleted', 'annotation_1', { old: 'gone' }))).toBe("Annotation 'gone' deleted.");
    expect(say(c('dep_added', 'abc-123'))).toBe("Dependency on 'abc-123' added.");
    expect(say(c('dep_deleted', 'abc-123'))).toBe("Dependency on 'abc-123' deleted.");
  });
});
