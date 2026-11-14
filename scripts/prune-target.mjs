#!/usr/bin/env node
// Frees the disk old builds of this workspace's own crates take in `target/`, keeping every dependency's.
//
// Cargo never deletes a build it no longer uses, and a crate's version is part of the hash in its file names, so each
// version bump leaves a whole copy of every workspace crate's builds (its library, test and binary builds, their
// incremental caches and fingerprints) beside the new one, in every profile and target. After a bump none of them is
// used again, and before one, deleting them costs only rebuilding our own crates: the dependencies, which take most of
// a build's time, stay. `just cleanup` runs this, and `just release` runs it after each bump.
//
//   node scripts/prune-target.mjs [target-dir]

import { execFileSync } from 'node:child_process';
import { existsSync, readdirSync, rmSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

/** The folders of a profile's build that hold one entry per crate build, named `<crate>-<hash>`. */
const KINDS = ['deps', 'incremental', '.fingerprint', 'build'];

/** The names the workspace's packages and targets (libraries, binaries, tests) are built under, as Cargo writes them. */
export function workspaceNames(metadata) {
  const names = new Set();
  for (const p of metadata.packages) {
    for (const n of [p.name, ...p.targets.map((t) => t.name)]) {
      names.add(n);
      names.add(n.replaceAll('-', '_'));
    }
  }
  return names;
}

/** Whether `entry` (`libtc_core-0123abcd.rlib`, `cli_exec-3dw0cu1n11sbs`) is a build of one of `names`. */
export function isWorkspaceBuild(entry, names) {
  const m = /^(?:lib)?(.+)-[0-9a-z]{13,16}(?:\..*)?$/.exec(entry);
  return !!m && names.has(m[1]);
}

/** The profile folders under `target` (`debug`, `release`, `aarch64-linux-android/android`, …): those with a `deps`. */
function profiles(target) {
  const found = [];
  const walk = (dir, depth) => {
    if (existsSync(join(dir, 'deps'))) found.push(dir);
    if (depth === 0) return;
    for (const e of readdirSync(dir, { withFileTypes: true })) {
      if (e.isDirectory() && !KINDS.includes(e.name)) walk(join(dir, e.name), depth - 1);
    }
  };
  if (existsSync(target)) walk(target, 2);
  return found;
}

/** Removes every build of `names` under `target`; returns how many entries and bytes went. */
export function prune(target, names) {
  let entries = 0;
  let bytes = 0;
  const size = (p) => {
    const s = statSync(p);
    if (!s.isDirectory()) return s.size;
    return readdirSync(p).reduce((sum, e) => sum + size(join(p, e)), 0);
  };
  for (const profile of profiles(target)) {
    for (const kind of KINDS) {
      const dir = join(profile, kind);
      if (!existsSync(dir)) continue;
      for (const e of readdirSync(dir)) {
        if (!isWorkspaceBuild(e, names)) continue;
        const p = join(dir, e);
        bytes += size(p);
        rmSync(p, { recursive: true, force: true });
        entries++;
      }
    }
  }
  return { entries, bytes };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const metadata = JSON.parse(
    execFileSync('cargo', ['metadata', '--no-deps', '--format-version', '1'], { encoding: 'utf8' }),
  );
  const target = process.argv[2] ?? metadata.target_directory;
  const { entries, bytes } = prune(target, workspaceNames(metadata));
  console.log(`Removed ${entries} builds of this workspace's crates (${(bytes / 1e9).toFixed(1)} GB) from ${target}.`);
}
