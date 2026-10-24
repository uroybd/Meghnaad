import { describe, expect, it } from 'vitest';
import { buildTags } from './tags';
import { activeTags, toggleTag } from './tagfilter';
import type { Row } from './types';

const row = (o: Partial<Row>): Row => ({ uuid: 'u', status: 'pending', tags: [], urgency: 0, due: null, ...o }) as Row;

describe('toggleTag', () => {
  it('adds a tag that is not in the filter, and removes one that is', () => {
    expect(toggleTag('', 'work')).toBe('+work');
    expect(toggleTag('project:Home +work', 'work')).toBe('project:Home');
    expect(toggleTag('project:Home', 'work')).toBe('project:Home +work');
  });
  it('replaces a contradicting -tag instead of keeping both', () => {
    expect(toggleTag('-work due:today', 'work')).toBe('due:today +work');
    expect(toggleTag('+a -b', 'b')).toBe('( +a or +b )');
  });
  it('joins a second tag to the first with or, and a third into the same group', () => {
    expect(toggleTag('project:Home +a', 'b')).toBe('project:Home ( +a or +b )');
    expect(toggleTag('project:Home ( +a or +b )', 'c')).toBe('project:Home ( +a or +b or +c )');
  });
  it('takes a tag out of the group, and the group away when one is left', () => {
    expect(toggleTag('( +a or +b or +c )', 'b')).toBe('( +a or +c )');
    expect(toggleTag('x ( +a or +b ) y', 'b')).toBe('x +a y');
    expect(toggleTag('( +a or +b )', 'a')).toBe('+b');
  });
  it('adds to the tags the person wrote together, which all have to be there, with and', () => {
    expect(toggleTag('+a +b', 'c')).toBe('+a +b +c');
  });
  it('keeps words with spaces quoted', () => {
    expect(toggleTag("'/two words/'", 'a')).toBe("'/two words/' +a");
  });
});

describe('activeTags', () => {
  it('lists only the required tags', () => {
    expect(activeTags('+a -b project:X +c.d')).toEqual(['a', 'c.d']);
  });
  it('lists the tags of a group', () => {
    expect(activeTags('project:X ( +a or +b ) -c')).toEqual(['a', 'b']);
  });
});

describe('buildTags', () => {
  it('groups tasks under each of their tags, by name, most urgent first, finished ones included', () => {
    const rows = [
      row({ uuid: '1', tags: ['b', 'a'], urgency: 1 }),
      row({ uuid: '2', tags: ['a'], urgency: 5, due: 10 }),
      row({ uuid: '3', tags: ['a'], status: 'completed' }),
    ];
    const t = buildTags(rows, 100);
    expect(t.map((e) => e.name)).toEqual(['a', 'b']);
    expect(t[0].tasks.map((r) => r.uuid)).toEqual(['2', '1', '3']);
    expect(t[0].overdue).toBe(1);
    expect(t[1].tasks).toHaveLength(1);
  });
  it('lists an extra tag even when nothing pending has it', () => {
    const t = buildTags([], 0, 'done-only');
    expect(t).toEqual([{ name: 'done-only', tasks: [], overdue: 0 }]);
  });
});
