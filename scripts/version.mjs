// What a release is called, in the forms the places that carry it need. The version lives in the root package.json; a tag
// `v1.2.3` has to match it (the Release workflow checks), and everything else is made from it: the web app's label in
// Settings, the Android app's versionName and versionCode, the version of the prebuilt package.
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { ROOT } from './deploy-lib.mjs';

const SEMVER = /^v?(\d+)\.(\d+)\.(\d+)(-[0-9A-Za-z.-]+)?$/;

/** `v1.2.3` or `1.2.3-rc.1` as `{ name, tag, code }`; `code` is Android's ever-rising integer (1.2.3 -> 1002003). */
export function parseVersion(text) {
  const m = SEMVER.exec(String(text).trim());
  if (!m) throw new Error(`"${text}" is not a version like 1.2.3 (or v1.2.3, or 1.2.3-rc.1)`);
  const [major, minor, patch] = [m[1], m[2], m[3]].map(Number);
  if (minor > 999 || patch > 999 || major > 2000) throw new Error(`"${text}" is too large for an Android version code`);
  const name = `${major}.${minor}.${patch}${m[4] ?? ''}`;
  return { name, tag: `v${name}`, code: major * 1_000_000 + minor * 1_000 + patch };
}

/** The version in the root package.json. */
export function currentVersion(root = ROOT) {
  return parseVersion(JSON.parse(readFileSync(join(root, 'package.json'), 'utf8')).version);
}

if (import.meta.url === `file://${process.argv[1]}`) {
  // `node scripts/version.mjs [name|tag|code]` for shell scripts.
  const v = currentVersion();
  console.log(v[process.argv[2] ?? 'name']);
}
