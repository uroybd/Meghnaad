#!/usr/bin/env node
// Downloads the static Meghnaad web app from a release, checks it against its published SHA-256, and unpacks it into
// ./site, ready for any static host. No dependencies: Node 20 or later and `tar`.
//
//   MEGHNAAD_VERSION  the release to take: `latest` (the default), or a version such as 0.8.0. Also read from the file
//                     `meghnaad-version` beside this script.
//   BASE_PATH         the folder the site is served from: `/` (the default), or `/<repo>/` on GitHub Pages.
//   MEGHNAAD_REPO     where the releases are (uroybd/Meghnaad).
//   OUT               where to put the site (site).
//   MEGHNAAD_DOWNLOAD  where the release's files are, in place of GitHub's (for testing this script).
//
// `config.json` beside this script, if there is one, is copied in: the defaults the Connect screen fills in (the tasks
// repository, branch and API address; never a token).
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));

/** The release asset for a version (the release workflow publishes it under this name). */
export const assetName = (version) => `meghnaad-static-${version}.tar.gz`;

/** `/`, `/repo/`: a leading and a trailing slash, whatever was given (GitHub's configure-pages gives `/repo`). */
export function basePath(given) {
  const trimmed = (given ?? '').trim().replace(/^\/+|\/+$/g, '');
  return trimmed ? `/${trimmed}/` : '/';
}

/** The page with its <base> pointing at the folder the site is served from. */
export function withBase(html, base) {
  if (!html.includes('<base href="/" />')) throw new Error('the page has no <base href="/" /> to set');
  return html.replace('<base href="/" />', `<base href="${base}" />`);
}

/** The version to take: the environment, else the `meghnaad-version` file, else the latest. */
export function wantedVersion(env = process.env, dir = HERE) {
  const file = join(dir, 'meghnaad-version');
  const fromFile = existsSync(file) ? readFileSync(file, 'utf8').trim() : '';
  return (env.MEGHNAAD_VERSION || fromFile || 'latest').replace(/^v/, '');
}

async function get(url, what) {
  const res = await fetch(url, { headers: { 'user-agent': 'meghnaad-static-template' } });
  if (!res.ok) throw new Error(`${what}: ${res.status} from ${url}`);
  return res;
}

async function main() {
  const repo = process.env.MEGHNAAD_REPO || 'uroybd/Meghnaad';
  const out = join(process.cwd(), process.env.OUT || 'site');
  let version = wantedVersion();
  if (version === 'latest' && process.env.MEGHNAAD_DOWNLOAD) throw new Error('MEGHNAAD_DOWNLOAD needs a version');
  if (version === 'latest') {
    const latest = await (
      await get(`https://api.github.com/repos/${repo}/releases/latest`, 'the latest release')
    ).json();
    version = String(latest.tag_name).replace(/^v/, '');
  }
  const asset = assetName(version);
  const base = process.env.MEGHNAAD_DOWNLOAD || `https://github.com/${repo}/releases/download/v${version}`;
  console.log(`Meghnaad ${version}: ${base}/${asset}`);
  const archive = Buffer.from(await (await get(`${base}/${asset}`, 'the site')).arrayBuffer());
  const published = (await (await get(`${base}/${asset}.sha256`, 'its checksum')).text()).trim().split(/\s+/)[0];
  const actual = createHash('sha256').update(archive).digest('hex');
  if (published !== actual) throw new Error(`the download does not match its checksum (${actual}, not ${published})`);

  rmSync(out, { recursive: true, force: true });
  mkdirSync(out, { recursive: true });
  const tarball = join(out, '..', `.${asset}`);
  writeFileSync(tarball, archive);
  try {
    execFileSync('tar', ['-xzf', tarball, '-C', out], { stdio: 'inherit' });
  } finally {
    rmSync(tarball, { force: true });
  }

  const folder = basePath(process.env.BASE_PATH);
  for (const page of ['index.html', '404.html']) {
    const p = join(out, page);
    if (existsSync(p)) writeFileSync(p, withBase(readFileSync(p, 'utf8'), folder));
  }
  if (existsSync(join(HERE, 'config.json'))) copyFileSync(join(HERE, 'config.json'), join(out, 'config.json'));
  console.log(`The site is in ${out}, served from ${folder}`);
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((e) => {
    console.error(`fetch-release: ${e.message}`);
    process.exit(1);
  });
}
