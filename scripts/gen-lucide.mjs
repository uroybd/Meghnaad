#!/usr/bin/env node
// Writes every Lucide icon the app can draw as one JSON file, for the Kotlin app (app/): the shapes of each icon from the
// installed @lucide/svelte, and the words each is found by from the same release's tags.json (what the icon picker searches).
// It also copies Taskwarrior's colour themes (web/public/themes) to the same resources, so the app picks one the way the web's
// taskrc tab does. A task command can name any Lucide icon (`circle-x`), and the phone has no CDN to fetch it from, so all of them travel with it.
//
//   node scripts/gen-lucide.mjs        writes app/shared/src/commonMain/resources/lucide.json and themes/*.theme
//
// Only a short list of shapes and attributes is kept, as web/src/lib/lucide.ts keeps from a fetched file.
import { copyFileSync, readdirSync, readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { ROOT } from './deploy-lib.mjs';

const PACKAGE = join(ROOT, 'node_modules', '@lucide', 'svelte');
const OUT = join(ROOT, 'app', 'shared', 'src', 'commonMain', 'resources', 'lucide.json');
const TAGS = new Set(['path', 'circle', 'rect', 'line', 'polyline', 'polygon', 'ellipse']);
const ATTRS = new Set([
  'd',
  'points',
  'cx',
  'cy',
  'r',
  'rx',
  'ry',
  'x',
  'y',
  'x1',
  'y1',
  'x2',
  'y2',
  'width',
  'height',
]);

const version = JSON.parse(readFileSync(join(PACKAGE, 'package.json'), 'utf8')).version;
const icons = {};
for (const file of readdirSync(join(PACKAGE, 'dist', 'icons'))) {
  if (!file.endsWith('.svelte')) continue;
  const source = readFileSync(join(PACKAGE, 'dist', 'icons', file), 'utf8');
  const found = /const iconData = (\{.*\});/.exec(source);
  if (!found) continue;
  const data = JSON.parse(found[1]);
  const shapes = [];
  for (const [tag, attrs] of data.node) {
    if (!TAGS.has(tag)) {
      shapes.length = 0;
      break;
    }
    shapes.push([tag, Object.fromEntries(Object.entries(attrs).filter(([k]) => ATTRS.has(k)))]);
  }
  if (shapes.length) icons[data.name] = shapes;
}
// An alias (`trash-2` is `trash`) is a small file that re-exports the icon under its new name: both names find it.
const aliases = {};
for (const file of readdirSync(join(PACKAGE, 'dist', 'icons'))) {
  if (!file.endsWith('.js')) continue;
  const to = /export \{ default \} from "\.\/([^"]+)\.svelte";/.exec(
    readFileSync(join(PACKAGE, 'dist', 'icons', file), 'utf8'),
  );
  const from = file.slice(0, -'.js'.length);
  if (to && icons[to[1]] && !icons[from]) aliases[from] = to[1];
}
for (const [from, to] of Object.entries(aliases)) icons[from] = icons[to];

const res = await fetch(`https://cdn.jsdelivr.net/npm/lucide-static@${version}/tags.json`);
if (!res.ok) throw new Error(`the tags of lucide-static@${version} could not be fetched (${res.status})`);
const all = await res.json();
const wordsOf = (name) => (Array.isArray(all[name]) ? all[name].filter((t) => typeof t === 'string') : []);
const tags = Object.fromEntries(Object.keys(icons).map((name) => [name, wordsOf(name)]));

mkdirSync(dirname(OUT), { recursive: true });
writeFileSync(OUT, JSON.stringify({ version, icons, tags }));
// The colour themes the taskrc tab offers: the same files the web serves.
const themes = join(ROOT, 'web', 'public', 'themes');
const themeOut = join(dirname(OUT), 'themes');
mkdirSync(themeOut, { recursive: true });
for (const f of readdirSync(themes)) if (f.endsWith('.theme')) copyFileSync(join(themes, f), join(themeOut, f));

console.log(`${Object.keys(icons).length} icons of Lucide ${version} -> ${OUT}`);
