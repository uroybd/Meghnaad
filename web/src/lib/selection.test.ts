import { beforeEach, describe, expect, it } from 'vitest';
import { selection } from './selection.svelte';

beforeEach(() => selection.clear());

describe('the selection', () => {
  it('ticks and unticks, in the order ticked', () => {
    selection.toggle('b');
    selection.toggle('a');
    selection.toggle('c');
    selection.toggle('a');
    expect(selection.uuids).toEqual(['b', 'c']);
    expect(selection.count).toBe(2);
    expect(selection.has('b') && !selection.has('a')).toBe(true);
  });

  it('ticks everything, and unticks everything when all were ticked', () => {
    selection.toggle('a');
    selection.toggleAll(['a', 'b', 'c']);
    expect(selection.uuids).toEqual(['a', 'b', 'c']);
    selection.toggleAll(['a', 'b', 'c']);
    expect(selection.uuids).toEqual([]);
  });

  it('drops what is no longer in the table', () => {
    selection.toggleAll(['a', 'b', 'c']);
    selection.keep(new Set(['b', 'z']));
    expect(selection.uuids).toEqual(['b']);
  });
});
