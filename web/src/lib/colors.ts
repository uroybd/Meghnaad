// Drawing Taskwarrior's colours on a page. The engine sends a style as palette indexes (0-15 the basic and bright
// colours, 16-231 the colour cube, 232-255 the grays); a terminal would show them in its own palette, here the
// basic sixteen are a soft palette that suits the page, the rest are the exact xterm colours, and a guard keeps
// whatever is chosen readable on the page it lands on.

import type { Resolved } from './types';

/** The soft sixteen, light page then dark page: black red green yellow blue magenta cyan white, then bright. */
const SOFT = {
  light: [
    '#3a3a36',
    '#cf5360',
    '#3a8f64',
    '#b7791f',
    '#3b82c4',
    '#9b5fc0',
    '#1f9aa5',
    '#9aa0a6',
    '#6b7280',
    '#e5484d',
    '#30a46c',
    '#c58b00',
    '#2f80ed',
    '#b36ae2',
    '#12b5c8',
    '#b8bec6',
  ],
  dark: [
    '#8b8b82',
    '#f08a8f',
    '#8fd1a5',
    '#e8c36a',
    '#86b8f0',
    '#c7a0ea',
    '#7fd6d9',
    '#d8dce0',
    '#9aa2ad',
    '#ff9aa0',
    '#9be3b0',
    '#f3d98a',
    '#a0c8ff',
    '#dab6f5',
    '#95e6ea',
    '#f2f4f6',
  ],
} as const;

/** The page behind a task row, and the text on it, per theme. */
const PAGE = { light: { bg: '#ffffff', text: '#1c1c1a' }, dark: { bg: '#1c1d17', text: '#e9e9e2' } } as const;

const CUBE = [0, 95, 135, 175, 215, 255];

type RGB = [number, number, number];

function toRgb(hex: string): RGB {
  const n = parseInt(hex.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

function toHex([r, g, b]: RGB): string {
  return (
    '#' +
    [r, g, b]
      .map((v) =>
        Math.round(Math.max(0, Math.min(255, v)))
          .toString(16)
          .padStart(2, '0'),
      )
      .join('')
  );
}

/** The colour of palette index `i`, as the page draws it. */
export function paletteHex(i: number, dark: boolean): string {
  if (i < 16) return SOFT[dark ? 'dark' : 'light'][i];
  if (i < 232) {
    const n = i - 16;
    return toHex([CUBE[Math.floor(n / 36)], CUBE[Math.floor(n / 6) % 6], CUBE[n % 6]]);
  }
  const v = 8 + 10 * (i - 232);
  return toHex([v, v, v]);
}

const mix = (a: string, b: string, weightOfA: number): string => {
  const [x, y] = [toRgb(a), toRgb(b)];
  return toHex([0, 1, 2].map((k) => x[k] * weightOfA + y[k] * (1 - weightOfA)) as RGB);
};

function luminance(hex: string): number {
  const lin = (c: number) => {
    const s = c / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  };
  const [r, g, b] = toRgb(hex);
  return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
}

export function contrast(a: string, b: string): number {
  const [x, y] = [luminance(a), luminance(b)].sort((p, q) => q - p);
  return (x + 0.05) / (y + 0.05);
}

/** `fg` moved toward the readable end (black on a light background, white on a dark one) until it can be read. */
export function readable(fg: string, bg: string, min = 3): string {
  if (contrast(fg, bg) >= min) return fg;
  // Toward whichever of black and white stands out more against the background.
  const target = contrast('#000000', bg) >= contrast('#ffffff', bg) ? '#000000' : '#ffffff';
  for (let w = 0.9; w >= 0; w -= 0.1) {
    const c = mix(fg, target, w);
    if (contrast(c, bg) >= min) return c;
  }
  return target;
}

/** A background colour: the soft basic colours are tinted into the page, the others used as they are. */
function backgroundHex(i: number, dark: boolean): string {
  const page = PAGE[dark ? 'dark' : 'light'].bg;
  if (i < 8) return mix(paletteHex(i, dark), page, dark ? 0.3 : 0.2);
  if (i < 16) return mix(paletteHex(i, dark), page, dark ? 0.45 : 0.32);
  return paletteHex(i, dark);
}

export interface Look {
  /** Text colour, or none to keep the page's. */
  color?: string;
  /** Background colour, or none. */
  background?: string;
  bold: boolean;
  underline: boolean;
}

/** What a style looks like on the page, over `under` (the background it lands on; the page's by default). */
export function look(
  style: Resolved | null | undefined,
  dark: boolean,
  under?: string,
  /** Draw the colours exactly (a palette, a sample) instead of softening backgrounds to suit the page. */
  solid = false,
): Look | null {
  if (!style) return null;
  const page = PAGE[dark ? 'dark' : 'light'];
  let fg = style.fg != null ? paletteHex(style.fg, dark) : undefined;
  let bg = style.bg != null ? (solid ? paletteHex(style.bg, dark) : backgroundHex(style.bg, dark)) : undefined;
  if (style.inverse) {
    // Swap: the text takes the background's colour (or the page's) and the background the text's (or the text's).
    [fg, bg] = [bg ?? page.bg, fg ?? page.text];
  }
  const behind = bg ?? under ?? page.bg;
  // A background with no text colour: the page's text, unless that would not show on it.
  if (bg && !fg) fg = readable(page.text, bg, 4.5);
  else if (fg) fg = readable(fg, behind);
  return { color: fg, background: bg, bold: !!style.bold, underline: !!style.underline };
}

/** The look as an inline `style` value. */
export function css(l: Look | null): string {
  if (!l) return '';
  const p: string[] = [];
  if (l.color) p.push(`color:${l.color}`);
  if (l.background) p.push(`background:${l.background}`);
  if (l.bold) p.push('font-weight:700');
  if (l.underline) p.push('text-decoration:underline');
  return p.join(';');
}

/** The look as custom properties, for a table row whose cells take them (see `tr.ruled` in ReportTable). */
export function rowVars(l: Look | null): string {
  if (!l) return '';
  const p: string[] = [];
  if (l.color) p.push(`--row-fg:${l.color}`);
  if (l.background) p.push(`--row-bg:${l.background}`);
  if (l.bold) p.push('--row-weight:700');
  if (l.underline) p.push('--row-deco:underline');
  return p.join(';');
}

/**
 * Every colour the engine sends (`calendar.today`, `history.add`, ...) as custom properties, to set on the page for
 * the charts to use: `--cc-calendar-today-fg`, `-bg`, `-w` (font weight) and `-u` (underline). A colour that is
 * not set leaves its properties out, so the chart's own fallback shows.
 */
export function chartProps(colors: Record<string, Resolved> | undefined, dark: boolean): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [name, style] of Object.entries(colors ?? {})) {
    const l = look(style, dark);
    if (!l) continue;
    const p = `--cc-${name.replace(/[^a-z0-9]+/gi, '-')}`;
    if (l.color) out[`${p}-fg`] = l.color;
    if (l.background) out[`${p}-bg`] = l.background;
    if (l.bold) out[`${p}-w`] = '700';
    if (l.underline) out[`${p}-u`] = 'underline';
  }
  return out;
}
