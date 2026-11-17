import { afterEach, describe, expect, it, vi } from 'vitest';
import { parsePath, placePath } from './route';

async function under(href: string) {
  const base = document.createElement('base');
  base.href = href;
  document.head.append(base);
  vi.resetModules();
  return import('./base');
}

afterEach(() => document.querySelectorAll('base').forEach((b) => b.remove()));

describe('addresses under a folder (a static site on GitHub Pages)', () => {
  it('adds and takes off the folder, and every place round-trips through it', async () => {
    const { BASE, fromAddress, toAddress } = await under('/tasks-site/');
    expect(BASE).toBe('/tasks-site/');
    expect(toAddress('/report/next')).toBe('/tasks-site/report/next');
    expect(toAddress('/')).toBe('/tasks-site/');
    expect(fromAddress('/tasks-site/report/next')).toBe('/report/next');
    expect(fromAddress('/tasks-site/')).toBe('/');
    for (const path of ['/', '/report/next', '/console', '/settings', '/boards', '/task/1234abcd']) {
      const place = parsePath(path);
      if (!place) continue;
      expect(parsePath(fromAddress(toAddress(placePath(place))))).toEqual(place);
    }
  });

  it('is the root for a Worker', async () => {
    const { BASE, fromAddress, toAddress } = await under('/');
    expect(BASE).toBe('/');
    expect(toAddress('/console')).toBe('/console');
    expect(fromAddress('/console')).toBe('/console');
  });
});
