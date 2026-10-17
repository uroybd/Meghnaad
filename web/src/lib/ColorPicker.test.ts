// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import { reactive } from '../reactive.svelte';
import ColorPicker from './ColorPicker.svelte';
import { CHART_COLOR_NAMES, defaultSeriesColor, namedColorHex } from './colors';

let app: ReturnType<typeof mount> | null = null;
afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
});

const swatches = () => [...document.querySelectorAll<HTMLButtonElement>('button.sw')];
const swatch = (label: string) => document.querySelector<HTMLButtonElement>(`button[aria-label="${label}"]`)!;
const draw = (props: { value?: string; position?: number }, onchange = vi.fn()) => {
  const p = reactive({ value: props.value, position: props.position ?? 0, label: 'Colour of series 1', onchange });
  app = mount(ColorPicker, { target: document.body, props: p });
  flushSync();
  return { onchange, p };
};

describe('the colour picker', () => {
  it('offers the default and each basic colour by name, drawn in the page’s palette', () => {
    draw({});
    // The default, then twelve.
    expect(swatches()).toHaveLength(1 + CHART_COLOR_NAMES.length);
    expect(swatch('red').style.background).not.toBe('');
    expect(swatch('red').title).toBe('red');
    // The default shows the colour the series gets at its place, and says which it is.
    expect(swatch('Default colour (blue)')).toBeTruthy();
    expect(document.querySelector<HTMLInputElement>('input[type=color]')).toBeTruthy();
  });

  it('marks what is chosen: the default, a name, or a value of its own', () => {
    draw({});
    expect(swatch('Default colour (blue)').getAttribute('aria-pressed')).toBe('true');
    expect(swatch('red').getAttribute('aria-pressed')).toBe('false');
    unmount(app!);
    document.body.innerHTML = '';

    draw({ value: 'Bright Red' });
    expect(swatch('bright red').getAttribute('aria-pressed')).toBe('true');
    expect(swatch('Default colour (blue)').getAttribute('aria-pressed')).toBe('false');
    unmount(app!);
    document.body.innerHTML = '';

    draw({ value: '#2da44e' });
    expect(swatches().every((b) => b.getAttribute('aria-pressed') === 'false')).toBe(true);
    expect(document.querySelector('input[type=color]')?.classList.contains('on')).toBe(true);
    expect((document.querySelector('input[type=color]') as HTMLInputElement).value).toBe('#2da44e');
  });

  it('reports a name when one is clicked, nothing for the default, and a value from the system picker', () => {
    const { onchange } = draw({ value: 'red' });
    swatch('cyan').click();
    expect(onchange).toHaveBeenLastCalledWith('cyan');
    swatch('Default colour (blue)').click();
    expect(onchange).toHaveBeenLastCalledWith(undefined);
    const any = document.querySelector<HTMLInputElement>('input[type=color]')!;
    any.value = '#112233';
    any.dispatchEvent(new Event('input', { bubbles: true }));
    expect(onchange).toHaveBeenLastCalledWith('#112233');
  });

  it('shows the default of the series’ place', () => {
    draw({ position: 2 });
    expect(swatch('Default colour (green)').style.background).not.toBe('');
    expect(defaultSeriesColor(2, false)).toBe(namedColorHex('green', false));
  });
});
