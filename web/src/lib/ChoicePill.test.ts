// @vitest-environment jsdom
// A UDA's choice drawn as a pill in a report cell (spec 0034): its own colour, grey when not among the choices.
import { afterEach, describe, expect, it } from 'vitest';
import { mount, tick, unmount } from 'svelte';
import ReportCell from './ReportCell.svelte';
import type { Column, Row } from './types';

let app: ReturnType<typeof mount> | null = null;
afterEach(() => {
  if (app) unmount(app);
  app = null;
});

const col = { name: 'size', label: 'Size', kind: 'string', spec: 'size' } as unknown as Column;
const row = { uuid: 'u', description: 'd', extra: {}, orphans: [] } as unknown as Row;

async function cell(c: { text: string; choice?: number }) {
  app = mount(ReportCell, { target: document.body, props: { col, c, row, entry: null, activeTags: [] } });
  await tick();
  return document.body.querySelector('.choice') as HTMLElement | null;
}

describe('a UDA value with choices', () => {
  it('is a pill in its own colour', async () => {
    const pill = await cell({ text: 'M', choice: 1 });
    expect(pill?.textContent).toBe('M');
    expect(pill?.getAttribute('style')).toContain('--hue:');
    expect(pill?.classList.contains('other')).toBe(false);
  });
  it('is a grey pill when the value is not among the choices', async () => {
    const pill = await cell({ text: 'XL', choice: -1 });
    expect(pill?.classList.contains('other')).toBe(true);
  });
  it('is text without choices', async () => {
    expect(await cell({ text: 'free' })).toBeNull();
  });
});
