// The static web app's build, release and deploy template agree: the asset's name, its checksum, the base folder.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync, spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { createServer } from 'node:http';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { ROOT } from './deploy-lib.mjs';
import { staticAsset } from './build-static.mjs';
import { assetName, basePath, wantedVersion, withBase } from '../templates/static/fetch-release.mjs';

const TEMPLATE = join(ROOT, 'templates', 'static');

test('the template asks for the asset the release publishes, with its checksum', () => {
  assert.equal(assetName('1.2.3'), staticAsset('1.2.3'));
  const release = readFileSync(join(ROOT, '.github', 'workflows', 'release.yml'), 'utf8');
  assert.ok(release.includes('"$RUNNER_TEMP/meghnaad-static-$VERSION.tar.gz"'));
  assert.ok(release.includes('"$RUNNER_TEMP/meghnaad-static-$VERSION.tar.gz.sha256"'));
  assert.ok(release.includes('"$RUNNER_TEMP/meghnaad-static-template.tar.gz"'));
  assert.equal(staticAsset('$VERSION'), 'meghnaad-static-$VERSION.tar.gz');
});

test('the base folder is always /…/, whatever GitHub Pages gives', () => {
  assert.equal(basePath(''), '/');
  assert.equal(basePath(undefined), '/');
  assert.equal(basePath('/tasks'), '/tasks/');
  assert.equal(basePath('tasks/'), '/tasks/');
  assert.equal(withBase('<head><base href="/" /></head>', '/tasks/'), '<head><base href="/tasks/" /></head>');
  assert.throws(() => withBase('<head></head>', '/tasks/'));
});

test('the page the build makes has the <base> the template sets', () => {
  const html = readFileSync(join(ROOT, 'web', 'index.html'), 'utf8');
  assert.ok(html.includes('<base href="/" />'));
});

test('the version comes from the environment, the file, or is the latest', () => {
  const dir = mkdtempSync(join(tmpdir(), 'meghnaad-tpl-'));
  try {
    assert.equal(wantedVersion({}, dir), 'latest');
    writeFileSync(join(dir, 'meghnaad-version'), 'v0.8.0\n');
    assert.equal(wantedVersion({}, dir), '0.8.0');
    assert.equal(wantedVersion({ MEGHNAAD_VERSION: '0.9.1' }, dir), '0.9.1');
    assert.equal(wantedVersion({}, TEMPLATE), 'latest');
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

/** Run the template's script against a local copy of a release; its exit code and output. */
async function fetchRelease(files, env) {
  const server = createServer((req, res) => {
    const name = req.url.split('/').pop();
    if (files[name]) res.writeHead(200).end(files[name]);
    else res.writeHead(404).end();
  });
  await new Promise((r) => server.listen(0, '127.0.0.1', r));
  const work = mkdtempSync(join(tmpdir(), 'meghnaad-fetch-'));
  try {
    const child = spawn('node', [join(TEMPLATE, 'fetch-release.mjs')], {
      cwd: work,
      env: { ...process.env, MEGHNAAD_DOWNLOAD: `http://127.0.0.1:${server.address().port}`, ...env },
    });
    let out = '';
    child.stdout.on('data', (d) => (out += d));
    child.stderr.on('data', (d) => (out += d));
    const code = await new Promise((r) => child.on('close', r));
    const read = (p) => readFileSync(join(work, 'site', p), 'utf8');
    return { code, out, read };
  } finally {
    server.close();
    setTimeout(() => rmSync(work, { recursive: true, force: true }), 500);
  }
}

function release(version) {
  const dir = mkdtempSync(join(tmpdir(), 'meghnaad-rel-'));
  mkdirSync(join(dir, 'site'));
  writeFileSync(join(dir, 'site', 'index.html'), '<head><base href="/" /></head>');
  writeFileSync(join(dir, 'site', '404.html'), '<head><base href="/" /></head>');
  const tarball = join(dir, 'site.tar.gz');
  execFileSync('tar', ['-czf', tarball, '-C', join(dir, 'site'), '.']);
  const bytes = readFileSync(tarball);
  rmSync(dir, { recursive: true, force: true });
  const sum = createHash('sha256').update(bytes).digest('hex');
  return { [assetName(version)]: bytes, [`${assetName(version)}.sha256`]: `${sum}  ${assetName(version)}\n` };
}

test('a release is downloaded, checked, unpacked and given its folder', async () => {
  const r = await fetchRelease(release('0.8.0'), { MEGHNAAD_VERSION: '0.8.0', BASE_PATH: '/tasks' });
  assert.equal(r.code, 0, r.out);
  assert.equal(r.read('index.html'), '<head><base href="/tasks/" /></head>');
  assert.equal(r.read('404.html'), '<head><base href="/tasks/" /></head>');
  assert.equal(r.read('config.json'), readFileSync(join(TEMPLATE, 'config.json'), 'utf8'));
});

test('a download that does not match its checksum is refused', async () => {
  const files = release('0.8.0');
  files[`${assetName('0.8.0')}.sha256`] = `${'0'.repeat(64)}  x\n`;
  const r = await fetchRelease(files, { MEGHNAAD_VERSION: '0.8.0' });
  assert.notEqual(r.code, 0);
  assert.match(r.out, /does not match its checksum/);
});
