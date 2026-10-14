#!/usr/bin/env node
// Makes the Android app's launcher icons from the web app's logo (web/public/logo.png), so the two
// look the same. Needs ImageMagick (`magick`). Run it again if the logo changes, and commit what it writes.
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { ROOT, capture, run } from './deploy-lib.mjs';

const LOGO = join(ROOT, 'web', 'public', 'logo.png');
const RES = join(ROOT, 'app', 'androidApp', 'src', 'main', 'res');
/** The logo's own background, so that the icon's background and the logo's meet without a seam. */
const BACKGROUND = '#0E0E0A';
const DENSITIES = { mdpi: 1, hdpi: 1.5, xhdpi: 2, xxhdpi: 3, xxxhdpi: 4 };

if (!capture('magick', ['--version']).ok) {
  console.error('ImageMagick is needed (brew install imagemagick)');
  process.exit(1);
}
const magick = (...args) => {
  const r = run('magick', args, { stdio: 'inherit' });
  if (r.status !== 0) process.exit(r.status ?? 1);
};

for (const [density, scale] of Object.entries(DENSITIES)) {
  const dir = join(RES, `mipmap-${density}`);
  mkdirSync(dir, { recursive: true });
  const icon = Math.round(48 * scale);
  const layer = Math.round(108 * scale);
  // The icon on a home screen that does not mask it (Android 7 and older).
  magick(LOGO, '-resize', `${icon}x${icon}`, join(dir, 'ic_launcher.png'));
  // The same in a circle.
  magick(
    LOGO,
    '-resize',
    `${icon}x${icon}`,
    '-background',
    BACKGROUND,
    '-alpha',
    'remove',
    '(',
    '-size',
    `${icon}x${icon}`,
    'xc:none',
    '-fill',
    'white',
    '-draw',
    `circle ${icon / 2},${icon / 2} ${icon / 2},0`,
    ')',
    '-compose',
    'DstIn',
    '-composite',
    join(dir, 'ic_launcher_round.png'),
  );
  // The adaptive icon's front layer: the logo inside the safe zone (the middle two thirds), on nothing.
  magick(
    '-size',
    `${layer}x${layer}`,
    'xc:none',
    '(',
    LOGO,
    '-resize',
    `${Math.round(layer * 0.72)}x${Math.round(layer * 0.72)}`,
    ')',
    '-gravity',
    'center',
    '-composite',
    join(dir, 'ic_launcher_foreground.png'),
  );
}

// Its back layer is the logo's own colour; the starter project's teal grid and drawn foreground are not used.
writeFileSync(
  join(RES, 'values', 'ic_launcher_background.xml'),
  `<?xml version="1.0" encoding="utf-8"?>\n<resources>\n    <color name="ic_launcher_background">${BACKGROUND}</color>\n</resources>\n`,
);

console.log('[icons] wrote the launcher icons from web/public/logo.png');
