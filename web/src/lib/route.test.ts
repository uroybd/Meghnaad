import { describe, expect, it } from 'vitest';
import { prefixFilter, prefixFromPath, taskLink, taskPath } from './route';

const uuid = '1a2b3c4d-5e6f-7a8b-9c0d-1e2f3a4b5c6d';

describe('a task’s address', () => {
  it('is the first 8 characters of its uuid', () => {
    expect(taskPath(uuid)).toBe('/task/1a2b3c4d');
    expect(taskLink(uuid, 'https://tasks.example.com')).toBe('https://tasks.example.com/task/1a2b3c4d');
  });

  it('is read back, from the short form or a longer prefix', () => {
    expect(prefixFromPath('/task/1a2b3c4d')).toBe('1a2b3c4d');
    expect(prefixFromPath('/task/1A2B3C4D/')).toBe('1a2b3c4d');
    expect(prefixFromPath(`/task/${uuid}`)).toBe(uuid);
    expect(prefixFromPath('/task/12345678')).toBe('12345678');
  });

  it('is not read from anything else', () => {
    for (const p of [
      '/',
      '/task',
      '/task/',
      '/task/zzzzzzzz',
      '/task/1a2b3c',
      '/tasks/1a2b3c4d',
      '/task/1a2b3c4d/edit',
      '/api/task/1a2b3c4d',
    ]) {
      expect(prefixFromPath(p), p).toBeNull();
    }
  });

  it('is looked up with an attribute filter so digits-only prefixes are not task numbers', () => {
    expect(prefixFilter('12345678')).toBe('uuid.startswith:12345678');
  });
});
