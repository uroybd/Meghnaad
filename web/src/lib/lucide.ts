// Icons by name (`circle-x`), for the task commands in settings.json, which can name any Lucide icon.
//
// The few the app uses itself are bundled (`icons.ts`) and found here by name. Any other is fetched from the CDN as
// the small SVG file it is, at a version pinned below, and turned into the shapes it draws with: only a short list of
// shapes and attributes is kept, so what comes back can be drawn but never run. The browser keeps the files (they
// never change for a version), and `lucideLib.svelte.ts` remembers the ones that were used.

import type { Component } from 'svelte';
import { ICON_NAME } from './appsettings';
import * as bundled from './icons';

/** The Lucide release the icons are fetched from. Changing it changes what an icon looks like, so it is pinned. */
export const LUCIDE_VERSION = '1.52.0';
export const CDN = `https://cdn.jsdelivr.net/npm/lucide-static@${LUCIDE_VERSION}`;

/** One shape of an icon: its element name and its attributes. */
export type Shape = [tag: string, attrs: Record<string, string>];

const TAGS = new Set(['path', 'circle', 'rect', 'line', 'polyline', 'polygon', 'ellipse']);
const NUMBERS = /^[-+0-9eE.,\s]*$/;
const PATH_DATA = /^[-+0-9eE.,\sMmLlHhVvCcSsQqTtAaZz]*$/;
const ATTRS: Record<string, RegExp> = {
  d: PATH_DATA,
  points: NUMBERS,
  cx: NUMBERS,
  cy: NUMBERS,
  r: NUMBERS,
  rx: NUMBERS,
  ry: NUMBERS,
  x: NUMBERS,
  y: NUMBERS,
  x1: NUMBERS,
  y1: NUMBERS,
  x2: NUMBERS,
  y2: NUMBERS,
  width: NUMBERS,
  height: NUMBERS,
};
const MAX_SHAPES = 40;
const MAX_VALUE = 4000;

/**
 * The shapes in an icon's SVG file, or null when it is not a plain icon: anything but the listed shapes (a script,
 * an image, a link, a style) refuses the whole file, and an attribute that isn't a plain number or path is dropped.
 */
export function svgToShapes(svg: string): Shape[] | null {
  const doc = new DOMParser().parseFromString(svg, 'image/svg+xml');
  const root = doc.documentElement;
  if (!root || root.localName !== 'svg' || doc.getElementsByTagName('parsererror').length) return null;
  const shapes: Shape[] = [];
  for (const el of Array.from(root.children)) {
    const tag = el.localName.toLowerCase();
    if (!TAGS.has(tag) || shapes.length >= MAX_SHAPES) return null;
    const attrs: Record<string, string> = {};
    for (const a of Array.from(el.attributes)) {
      const ok = ATTRS[a.name];
      if (ok && a.value.length <= MAX_VALUE && ok.test(a.value)) attrs[a.name] = a.value;
    }
    shapes.push([tag, attrs]);
  }
  return shapes.length ? shapes : null;
}

/** PascalCase to Lucide's names: `ArrowRightToLine` is `arrow-right-to-line`, `Trash2` is `trash-2`. */
const kebab = (s: string) =>
  s
    .replace(/([a-z])([A-Z])/g, '$1-$2')
    .replace(/([a-zA-Z])(\d)/g, '$1-$2')
    .replace(/(\d)([A-Z])/g, '$1-$2')
    .toLowerCase();

/** The icons the app already carries, by Lucide name: they need no fetch. */
export const bundledIcons: ReadonlyMap<string, Component> = new Map(
  Object.entries(bundled)
    .filter(([, v]) => typeof v === 'function')
    .map(([k, v]) => [kebab(k), v as unknown as Component]),
);

/** The result of asking for an icon: its shapes, or why there are none. */
export type Fetched = { shapes: Shape[] } | { missing: true } | { failed: true };

/** One icon from the CDN. A 404 is an icon that doesn't exist; anything else is only not available right now. */
export async function fetchIcon(name: string, get: typeof fetch = fetch): Promise<Fetched> {
  if (!ICON_NAME.test(name)) return { missing: true };
  try {
    const res = await get(`${CDN}/icons/${name}.svg`);
    if (res.status === 404) return { missing: true };
    if (!res.ok) return { failed: true };
    const shapes = svgToShapes(await res.text());
    return shapes ? { shapes } : { failed: true };
  } catch {
    return { failed: true };
  }
}

/** Every icon's name with the words it is found by (`x`: close, cancel, …). About 50 KB compressed. */
export async function fetchCatalog(get: typeof fetch = fetch): Promise<Record<string, string[]> | null> {
  try {
    const res = await get(`${CDN}/tags.json`);
    if (!res.ok) return null;
    const data: unknown = await res.json();
    if (!data || typeof data !== 'object' || Array.isArray(data)) return null;
    const out: Record<string, string[]> = {};
    for (const [name, tags] of Object.entries(data)) {
      if (ICON_NAME.test(name))
        out[name] = Array.isArray(tags) ? tags.filter((t): t is string => typeof t === 'string') : [];
    }
    return Object.keys(out).length ? out : null;
  } catch {
    return null;
  }
}
