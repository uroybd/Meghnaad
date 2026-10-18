import { beforeEach, describe, expect, it } from 'vitest';
import { contextWord, loadChoices, saveChoices, withContext } from './pageContext';

beforeEach(() => localStorage.clear());

describe('a command in a page’s context', () => {
  it('is sent as Taskwarrior’s own override', () => {
    expect(contextWord('work')).toBe('rc.context=work');
    expect(withContext({ args: ['+x', 'export'] }, 'work')).toEqual({ args: ['rc.context=work', '+x', 'export'] });
    expect(withContext({ line: 'next' }, 'none')).toEqual({ line: 'rc.context=none next' });
  });

  it('is left alone when the page follows the active context', () => {
    const input = { args: ['next'] };
    expect(withContext(input, undefined)).toBe(input);
  });
});

describe('the choices kept in the browser', () => {
  it('come back as saved, and a damaged value is nothing', () => {
    saveChoices({ tasks: 'work', 'board:b': 'none' });
    expect(loadChoices()).toEqual({ tasks: 'work', 'board:b': 'none' });
    localStorage.setItem('tw-web-page-contexts', '[1,2]');
    expect(loadChoices()).toEqual({});
    localStorage.setItem('tw-web-page-contexts', '{"a":"x","b":3,"c":""}');
    expect(loadChoices()).toEqual({ a: 'x' });
    localStorage.setItem('tw-web-page-contexts', 'not json');
    expect(loadChoices()).toEqual({});
  });
});
