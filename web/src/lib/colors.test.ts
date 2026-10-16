import { describe, expect, it } from 'vitest';
import {
  CHART_COLOR_NAMES,
  chartColor,
  chartProps,
  contrast,
  css,
  defaultSeriesColor,
  defaultSeriesName,
  look,
  namedColorHex,
  paletteHex,
  readable,
  rowVars,
} from './colors';

describe('paletteHex', () => {
  it('draws the colour cube and the grays exactly, as xterm does', () => {
    expect(paletteHex(16, false)).toBe('#000000'); // rgb000
    expect(paletteHex(196, false)).toBe('#ff0000'); // rgb500
    expect(paletteHex(46, true)).toBe('#00ff00'); // rgb050
    expect(paletteHex(231, false)).toBe('#ffffff'); // rgb555
    expect(paletteHex(67, false)).toBe('#5f87af'); // rgb123
    expect(paletteHex(232, false)).toBe('#080808'); // gray0
    expect(paletteHex(244, false)).toBe('#808080'); // gray12
    expect(paletteHex(255, false)).toBe('#eeeeee'); // gray23
  });

  it('draws the basic sixteen in a softer palette that follows the page', () => {
    for (const i of [1, 2, 3, 4, 5, 6]) expect(paletteHex(i, false)).not.toBe(paletteHex(i, true));
    expect(paletteHex(1, false)).not.toBe('#ff0000');
    expect(paletteHex(1, false)).toBe('#cf5360');
  });
});

describe('readable', () => {
  it('leaves a colour that can be read alone and moves one that cannot', () => {
    expect(readable('#cf5360', '#ffffff')).toBe('#cf5360');
    // Near-white text on a white page is pulled toward black until it shows.
    const c = readable('#eeeeee', '#ffffff');
    expect(contrast(c, '#ffffff')).toBeGreaterThanOrEqual(3);
    // And the other way round on a dark page.
    const d = readable('#080808', '#1c1d17');
    expect(contrast(d, '#1c1d17')).toBeGreaterThanOrEqual(3);
  });
});

describe('look', () => {
  it('has nothing to say for no style', () => {
    expect(look(undefined, false)).toBeNull();
    expect(css(look(null, false))).toBe('');
  });

  it('a foreground alone is the colour, readable on the page', () => {
    const l = look({ fg: 255 }, false)!; // color255, near white: a terminal's choice for a dark screen
    expect(l.background).toBeUndefined();
    expect(contrast(l.color!, '#ffffff')).toBeGreaterThanOrEqual(3);
  });

  it('a background alone gets text that can be read on it', () => {
    for (const dark of [false, true]) {
      for (const bg of [2, 4, 10, 166, 17, 234]) {
        const l = look({ bg }, dark)!;
        expect(contrast(l.color!, l.background!)).toBeGreaterThanOrEqual(4.5);
      }
    }
  });

  it('the basic backgrounds are tinted into the page, but a swatch shows them as they are', () => {
    const tinted = look({ bg: 1 }, false)!.background!;
    const solid = look({ bg: 1 }, false, undefined, true)!.background!;
    expect(solid).toBe(paletteHex(1, false));
    expect(tinted).not.toBe(solid);
    // The cube is never tinted.
    expect(look({ bg: 196 }, false)!.background).toBe('#ff0000');
  });

  it('inverse swaps the two', () => {
    const l = look({ fg: 196, bg: 46, inverse: true }, false, undefined, true)!;
    expect(l.background).toBe('#ff0000');
    expect(l.color).not.toBeUndefined();
  });

  it('bold and underline pass through', () => {
    const l = look({ bold: true, underline: true, fg: 1 }, false)!;
    expect(css(l)).toContain('font-weight:700');
    expect(css(l)).toContain('text-decoration:underline');
  });
});

describe('rowVars', () => {
  it('turns a look into the custom properties a table row hands its cells', () => {
    expect(rowVars(null)).toBe('');
    const v = rowVars(look({ bold: true, fg: 1 }, false));
    expect(v).toContain('--row-fg:');
    expect(v).toContain('--row-weight:700');
    expect(v).not.toContain('--row-bg');
  });
});

describe('chartProps', () => {
  it('names each colour for the charts, with dots as dashes, and leaves out what is not set', () => {
    const p = chartProps(
      { 'calendar.today': { bold: true, bg: 4 }, 'calendar.due.today': { fg: 3 }, 'history.add': {} },
      false,
    );
    expect(Object.keys(p).sort()).toEqual([
      '--cc-calendar-due-today-fg',
      '--cc-calendar-today-bg',
      '--cc-calendar-today-fg',
      '--cc-calendar-today-w',
    ]);
    expect(chartProps(undefined, false)).toEqual({});
  });
});

describe('the colours a chart is given by name', () => {
  it('are the basic colours of the page’s palette, and follow a light and a dark page', () => {
    expect(namedColorHex('red', false)).toBe(paletteHex(1, false));
    expect(namedColorHex(' Bright Blue ', true)).toBe(paletteHex(12, true));
    expect(namedColorHex('red', true)).not.toBe(namedColorHex('red', false));
    expect(namedColorHex('puce', false)).toBeNull();
  });

  it('are offered without black and white, which would vanish on one page or the other', () => {
    expect(CHART_COLOR_NAMES).toHaveLength(12);
    expect(CHART_COLOR_NAMES).not.toContain('black');
    expect(CHART_COLOR_NAMES).not.toContain('white');
    for (const n of CHART_COLOR_NAMES) expect(namedColorHex(n, false), n).toMatch(/^#[0-9a-f]{6}$/);
  });

  it('can be a name or a value, and nothing else', () => {
    expect(chartColor('green', false)).toBe(paletteHex(2, false));
    expect(chartColor('#2da44e', true)).toBe('#2da44e');
    expect(chartColor('puce', false)).toBeUndefined();
    expect(chartColor(undefined, false)).toBeUndefined();
  });

  it('are what a series takes when it names none: blue first, and round again after the last', () => {
    expect(defaultSeriesColor(0, false)).toBe(namedColorHex('blue', false));
    expect(defaultSeriesName(1)).toBe('red');
    expect(defaultSeriesColor(9, false)).toBe(defaultSeriesColor(0, false));
  });
});
