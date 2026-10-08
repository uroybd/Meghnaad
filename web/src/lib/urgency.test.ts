import { describe as suite, expect, it } from 'vitest';
import { AGE_MAX, buildKey, describe, effective, sameOverrides, sortKeys, validName, validUda, withValue } from './urgency';

const defaults = { 'urgency.due.coefficient': 12, 'urgency.user.tag.next.coefficient': 15 };

suite('describe', () => {
  it('names built-in terms and the age cap', () => {
    expect(describe('urgency.due.coefficient')).toMatchObject({ kind: 'term', label: 'Due date' });
    expect(describe(AGE_MAX)).toMatchObject({ kind: 'term', label: 'Age cap (days)' });
  });

  it('says a project coefficient reaches its sub-projects', () => {
    const d = describe('urgency.user.project.Home.Kitchen.coefficient');
    expect(d).toMatchObject({ kind: 'project', name: 'Home.Kitchen' });
    expect(d.hint).toContain('Home.Kitchen.*');
  });

  it('reads tags, keywords and UDA values', () => {
    expect(describe('urgency.user.tag.next.coefficient')).toMatchObject({ kind: 'tag', name: 'next' });
    expect(describe('urgency.user.keyword.call.coefficient')).toMatchObject({ kind: 'keyword', name: 'call' });
    expect(describe('urgency.uda.priority.H.coefficient')).toMatchObject({ kind: 'uda', label: 'priority = H' });
    expect(describe('urgency.uda.estimate.coefficient')).toMatchObject({ kind: 'uda', label: 'estimate has a value' });
  });
});

suite('keys', () => {
  it('builds the keys Taskwarrior reads', () => {
    expect(buildKey('project', 'Home')).toBe('urgency.user.project.Home.coefficient');
    expect(buildKey('tag', 'next')).toBe('urgency.user.tag.next.coefficient');
    expect(buildKey('keyword', 'call')).toBe('urgency.user.keyword.call.coefficient');
    expect(buildKey('uda', 'estimate.huge')).toBe('urgency.uda.estimate.huge.coefficient');
  });

  it('refuses names that could not survive a taskrc', () => {
    for (const bad of ['', 'a b', 'a=b', 'a#b', 'x.coefficient']) expect(validName(bad)).toBe(false);
    expect(validName('Home.Kitchen')).toBe(true);
    expect(validUda('estimate')).toBe(true);
    expect(validUda('estimate.huge')).toBe(true);
    expect(validUda('bad-name')).toBe(false);
  });

  it('lists built-in terms before custom settings, in Taskwarrior order', () => {
    const keys = [
      'urgency.user.tag.b.coefficient',
      'urgency.user.project.A.coefficient',
      'urgency.age.coefficient',
      AGE_MAX,
      'urgency.project.coefficient',
    ];
    expect(sortKeys(keys)).toEqual([
      'urgency.project.coefficient',
      'urgency.age.coefficient',
      AGE_MAX,
      'urgency.user.project.A.coefficient',
      'urgency.user.tag.b.coefficient',
    ]);
  });
});

suite('overrides', () => {
  it('falls back to the built-in value', () => {
    expect(effective('urgency.due.coefficient', {}, defaults)).toBe(12);
    expect(effective('urgency.due.coefficient', { 'urgency.due.coefficient': 3 }, defaults)).toBe(3);
    expect(effective('urgency.user.tag.x.coefficient', {}, defaults)).toBeUndefined();
  });

  it('drops an override that equals the default, keeps others, and never mutates', () => {
    const start = { 'urgency.due.coefficient': 3 };
    expect(withValue(start, defaults, 'urgency.due.coefficient', 12)).toEqual({});
    expect(withValue(start, defaults, 'urgency.due.coefficient', 0)).toEqual({ 'urgency.due.coefficient': 0 });
    expect(withValue({}, defaults, 'urgency.user.tag.x.coefficient', 2)).toEqual({ 'urgency.user.tag.x.coefficient': 2 });
    expect(start).toEqual({ 'urgency.due.coefficient': 3 });
  });

  it('compares override sets', () => {
    expect(sameOverrides({ a: 1 }, { a: 1 })).toBe(true);
    expect(sameOverrides({ a: 1 }, { a: 2 })).toBe(false);
    expect(sameOverrides({ a: 1 }, { a: 1, b: 2 })).toBe(false);
  });
});
