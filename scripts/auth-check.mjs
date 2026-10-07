#!/usr/bin/env node
// Does the Worker really refuse everything that isn't a valid Cloudflare Access token?
//
// Starts a mock Access (the JWKS endpoint the Worker fetches signing keys from), runs the Worker
// with the local auth bypass OFF, and sends it good and bad tokens. Self-contained: its own ports
// and storage, never touches a dev server you have running. Needs only Node and a built Worker:
//   (cd crates/worker && worker-build --release) && node scripts/auth-check.mjs
import { createServer, request as httpRequest } from 'node:http';
import { spawn } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const SHIM = path.join(ROOT, 'crates/worker/build/worker/shim.mjs');
const JWKS_PORT = 9911, API_PORT = 9912, BYPASS_PORT = 9913;
const AUD = 'test-audience-tag';
const TEAM = `http://127.0.0.1:${JWKS_PORT}`; // the Worker's issuer must equal TEAM_DOMAIN
if (!fs.existsSync(SHIM)) { console.error('Build the Worker first: (cd crates/worker && worker-build --release)'); process.exit(2); }

// ---- tokens -------------------------------------------------------------------------------
const b64 = (x) => Buffer.from(typeof x === 'string' ? x : JSON.stringify(x)).toString('base64url');
const rsa = () => crypto.generateKeyPairSync('rsa', { modulusLength: 2048 });
const good = rsa(), other = rsa();
const jwk = (pub, kid) => ({ ...pub.export({ format: 'jwk' }), kid, alg: 'RS256', use: 'sig' });
const now = () => Math.floor(Date.now() / 1000);
const claims = (over = {}) => ({ aud: [AUD], iss: TEAM, exp: now() + 600, iat: now(), email: 'me@example.com', ...over });
function mint({ key = good.privateKey, kid = 'k1', header = {}, payload = claims(), sign = true } = {}) {
  const h = b64({ alg: 'RS256', typ: 'JWT', kid, ...header });
  const p = b64(payload);
  const sig = sign ? crypto.sign('sha256', Buffer.from(`${h}.${p}`), key).toString('base64url') : '';
  return `${h}.${p}.${sig}`;
}

// ---- mock Access --------------------------------------------------------------------------
let jwksHits = 0;
const jwks = createServer((req, res) => {
  if (req.url === '/cdn-cgi/access/certs') {
    jwksHits++;
    res.setHeader('content-type', 'application/json');
    res.end(JSON.stringify({ keys: [jwk(good.publicKey, 'k1')], public_cert: {} }));
  } else { res.statusCode = 404; res.end(); }
});
await new Promise((r) => jwks.listen(JWKS_PORT, '127.0.0.1', r));

// ---- the Worker, twice: without and with the dev bypass ------------------------------------
const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'auth-check-'));
const procs = [];
function startWorker(name, port, devVars) {
  const dir = path.join(tmp, name);
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(path.join(dir, 'wrangler.jsonc'), JSON.stringify({
    name: `auth-check-${name}`, main: SHIM, compatibility_date: '2026-10-01',
    r2_buckets: [{ binding: 'TASKS', bucket_name: 'auth-check' }],
    vars: { TEAM_DOMAIN: TEAM, POLICY_AUD: AUD },
  }));
  fs.writeFileSync(path.join(dir, '.dev.vars'), devVars);
  const p = spawn('npx', ['wrangler', 'dev', '--config', path.join(dir, 'wrangler.jsonc'), '--port', String(port),
    '--ip', '127.0.0.1', '--persist-to', path.join(dir, 'state'), '--log-level', 'error'],
    { cwd: ROOT, detached: true, stdio: ['ignore', 'pipe', 'pipe'] });
  p.stderr.on('data', () => {}); p.stdout.on('data', () => {});
  procs.push(p);
}
const cleanup = () => { for (const p of procs) try { process.kill(-p.pid, 'SIGKILL'); } catch {} jwks.close(); fs.rmSync(tmp, { recursive: true, force: true }); };
process.on('exit', cleanup); process.on('SIGINT', () => process.exit(130));
startWorker('strict', API_PORT, 'TC_ENCRYPTION_SECRET=auth-check-secret\n');
startWorker('bypass', BYPASS_PORT, 'TC_ENCRYPTION_SECRET=auth-check-secret\nDEV_AUTH_BYPASS=1\n');

// ---- requests (http.request, so we can set Host and Origin like a browser/proxy would) -------
function send(port, { method = 'GET', url = '/api/health', token, cookie, host, origin, body } = {}) {
  return new Promise((resolve) => {
    const headers = {};
    if (token !== undefined) headers['cf-access-jwt-assertion'] = token;
    if (host) headers.host = host;
    if (cookie) headers.cookie = cookie;
    if (origin !== undefined) headers.origin = origin;
    if (body !== undefined) { headers['content-type'] = 'application/json'; }
    const r = httpRequest({ host: '127.0.0.1', port, path: url, method, headers, timeout: 20000 }, (res) => {
      let data = ''; res.on('data', (c) => (data += c)); res.on('end', () => resolve({ status: res.statusCode, data }));
    });
    r.on('error', (e) => resolve({ status: 0, data: String(e) }));
    r.on('timeout', () => { r.destroy(); resolve({ status: -1, data: 'timeout' }); });
    if (body !== undefined) r.write(JSON.stringify(body));
    r.end();
  });
}
async function waitUp(port) {
  for (let i = 0; i < 150; i++) { const r = await send(port); if (r.status > 0) return; await new Promise((x) => setTimeout(x, 1000)); }
  throw new Error(`Worker on ${port} never came up`);
}
await Promise.all([waitUp(API_PORT), waitUp(BYPASS_PORT)]);

let failed = 0;
const expect = (name, got, want) => {
  const ok = got.status === want;
  if (!ok) failed++;
  console.log(`${ok ? 'PASS' : 'FAIL'}  ${name}  (${got.status}${ok ? '' : `, wanted ${want}: ${got.data.slice(0, 120)}`})`);
};

console.log('== token checks (dev bypass OFF)');
const s = (o) => send(API_PORT, o);
expect('valid token is accepted', await s({ token: mint() }), 200);
expect('valid token, audience as a plain string', await s({ token: mint({ payload: claims({ aud: AUD }) }) }), 200);
expect('no token at all', await s({}), 403);
expect('empty token', await s({ token: '' }), 403);
expect('garbage token', await s({ token: 'not.a.jwt' }), 403);
expect('two segments only', await s({ token: 'abc.def' }), 403);
expect('payload tampered after signing (email swapped)', await s({ token: (() => { const t = mint().split('.'); t[1] = b64(claims({ email: 'attacker@evil.test' })); return t.join('.'); })() }), 403);
expect('signed by a different key, same kid', await s({ token: mint({ key: other.privateKey }) }), 403);
expect('unknown key id', await s({ token: mint({ kid: 'nope' }) }), 403);
expect('no key id', await s({ token: (() => { const h = b64({ alg: 'RS256', typ: 'JWT' }); const p = b64(claims()); return `${h}.${p}.${crypto.sign('sha256', Buffer.from(`${h}.${p}`), good.privateKey).toString('base64url')}`; })() }), 403);
expect('wrong audience', await s({ token: mint({ payload: claims({ aud: ['someone-elses-app'] }) }) }), 403);
expect('audience missing', await s({ token: mint({ payload: (({ aud, ...r }) => r)(claims()) }) }), 403);
expect('wrong issuer', await s({ token: mint({ payload: claims({ iss: 'https://evil.cloudflareaccess.com' }) }) }), 403);
expect('expired', await s({ token: mint({ payload: claims({ exp: now() - 10 }) }) }), 403);
expect('no expiry claim', await s({ token: mint({ payload: (({ exp, ...r }) => r)(claims()) }) }), 403);
expect('not valid yet (nbf in the future)', await s({ token: mint({ payload: claims({ nbf: now() + 3600 }) }) }), 403);
expect('alg "none", unsigned', await s({ token: mint({ header: { alg: 'none' }, sign: false }) }), 403);
expect('alg "none" with a real-looking signature', await s({ token: mint({ header: { alg: 'none' } }) }), 403);
expect('alg confusion: HS256 keyed with the public key', await s({ token: (() => {
  const h = b64({ alg: 'HS256', typ: 'JWT', kid: 'k1' }), p = b64(claims());
  const secret = good.publicKey.export({ type: 'spki', format: 'pem' });
  return `${h}.${p}.${crypto.createHmac('sha256', secret).update(`${h}.${p}`).digest('base64url')}`;
})() }), 403);
expect('a valid token in the CF_Authorization cookie alone is not enough (only the header Access injects counts)', await s({ cookie: `CF_Authorization=${mint()}` }), 403);
expect('every write route is protected too', (await Promise.all([
  s({ method: 'POST', url: '/api/cli', body: { line: 'count' } }),
  s({ method: 'PUT', url: '/api/config/taskrc', body: {} }),
  s({ method: 'POST', url: '/api/config/taskrc/restore' }),
  s({ url: '/api/config' }), s({ url: '/api/config/taskrc' }),
])).every((r) => r.status === 403) ? { status: 403, data: '' } : { status: 200, data: 'a protected route answered' }, 403);
expect('a valid token reaches a write route', await s({ method: 'POST', url: '/api/cli', token: mint(), body: { line: 'count' } }), 200);

console.log('== bypass flag (DEV_AUTH_BYPASS=1 set on the Worker)');
const b = (o) => send(BYPASS_PORT, o);
expect('loopback + bypass: no token needed (local development)', await b({}), 200);
// Does the Worker see the spoofed Host as the request host? If so the bypass must not apply.
const hostHonoured = (await send(BYPASS_PORT, { host: 'tasks.example.com', url: '/api/health' })).status;
if (hostHonoured === 403) {
  expect('non-loopback host + bypass flag: bypass is ignored', { status: hostHonoured, data: '' }, 403);
  expect('non-loopback host + valid token: still works', await b({ host: 'tasks.example.com', token: mint() }), 200);
  expect('non-loopback host: cross-site write refused', await b({ method: 'POST', url: '/api/cli', host: 'tasks.example.com', token: mint(), origin: 'https://evil.example', body: { line: 'count' } }), 403);
  expect('non-loopback host: opaque "null" origin refused', await b({ method: 'POST', url: '/api/cli', host: 'tasks.example.com', token: mint(), origin: 'null', body: { line: 'count' } }), 403);
  expect('non-loopback host: same-origin write allowed', await b({ method: 'POST', url: '/api/cli', host: 'tasks.example.com', token: mint(), origin: 'http://tasks.example.com', body: { line: 'count' } }), 200);
  expect('non-loopback host: a script with no Origin is allowed', await b({ method: 'POST', url: '/api/cli', host: 'tasks.example.com', token: mint(), body: { line: 'count' } }), 200);
} else {
  console.log(`NOTE  the local runtime doesn't pass a spoofed Host through (got ${hostHonoured}); the host-based checks are covered by unit tests only`);
}

console.log(`\nsigning keys were fetched ${jwksHits} time(s) in total (cached between requests)`);
console.log(failed ? `\n${failed} CHECK(S) FAILED` : '\nALL AUTH CHECKS PASSED');
process.exit(failed ? 1 : 0);
