// Shared by `npm run setup` and `npm run deploy`.
//
// Your deployment choices (bucket, custom domain, Access settings) live in `.deploy.vars`, which is
// git-ignored. `buildConfig` overlays them on `wrangler.jsonc` into `wrangler.deploy.jsonc` (also
// git-ignored), so the tracked config never needs personal edits and `git pull` never conflicts.
import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

export const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
export const VARS_FILE = join(ROOT, '.deploy.vars');
export const BASE_CONFIG = join(ROOT, 'wrangler.jsonc');
export const DEPLOY_CONFIG = join(ROOT, 'wrangler.deploy.jsonc');

export const KEYS = ['R2_BUCKET', 'CUSTOM_DOMAIN', 'TEAM_DOMAIN', 'POLICY_AUD', 'WORKER_NAME'];

const PATTERNS = {
  R2_BUCKET: [/^[a-z0-9][a-z0-9-]{1,61}[a-z0-9]$/, 'a bucket name: 3-63 lowercase letters, digits or hyphens'],
  CUSTOM_DOMAIN: [/^(?=.{4,253}$)([a-z0-9]([a-z0-9-]*[a-z0-9])?\.)+[a-z]{2,}$/, 'a hostname such as tasks.example.com (no https://)'],
  TEAM_DOMAIN: [/^https:\/\/[a-z0-9-]+\.cloudflareaccess\.com$/, 'https://<your-team>.cloudflareaccess.com'],
  POLICY_AUD: [/^[0-9a-f]{64}$/, 'the 64-character Application Audience tag'],
  WORKER_NAME: [/^[a-z0-9][a-z0-9-]{0,62}$/, 'a Worker name: lowercase letters, digits or hyphens'],
};

/** Why `value` is not acceptable for `key`, or null when it is (an empty value means "not set"). */
export function problem(key, value) {
  if (!value) return null;
  const [re, want] = PATTERNS[key];
  return re.test(value) ? null : `${key} should be ${want}`;
}

/** Remove // and /* *\/ comments and trailing commas without touching strings (URLs contain //). */
export function stripJsonc(text) {
  let out = '';
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (c === '"') {
      let j = i + 1;
      while (j < text.length && text[j] !== '"') j += text[j] === '\\' ? 2 : 1;
      out += text.slice(i, j + 1);
      i = j;
    } else if (c === '/' && text[i + 1] === '/') {
      while (i < text.length && text[i] !== '\n') i++;
      out += '\n';
    } else if (c === '/' && text[i + 1] === '*') {
      i = text.indexOf('*/', i + 2);
      if (i < 0) break;
      i++;
    } else out += c;
  }
  return out.replace(/,(\s*[}\]])/g, '$1');
}

export function readSettings(env = process.env) {
  const out = Object.fromEntries(KEYS.map((k) => [k, '']));
  if (existsSync(VARS_FILE)) {
    for (const line of readFileSync(VARS_FILE, 'utf8').split('\n')) {
      const m = /^\s*([A-Z0-9_]+)\s*=\s*(.*?)\s*$/.exec(line);
      if (m && !line.trim().startsWith('#') && KEYS.includes(m[1])) out[m[1]] = m[2];
    }
  }
  for (const k of KEYS) if (env[k]) out[k] = env[k].trim();
  return out;
}

export function writeSettings(s) {
  const lines = KEYS.filter((k) => s[k]).map((k) => `${k}=${s[k]}`);
  writeFileSync(
    VARS_FILE,
    '# Your deployment settings (git-ignored). Read by `npm run deploy`; edited by `npm run setup`.\n' +
      '# R2_BUCKET     existing or new bucket your `task` CLI syncs to (default: the one in wrangler.jsonc)\n' +
      '# CUSTOM_DOMAIN optional, e.g. tasks.example.com (the zone must be on your Cloudflare account)\n' +
      '# TEAM_DOMAIN / POLICY_AUD   from Cloudflare Access\n' +
      '# WORKER_NAME   optional, to run several copies\n' +
      lines.join('\n') + '\n',
  );
}

/** Settings problems as readable strings. */
export const problems = (s) => KEYS.flatMap((k) => problem(k, s[k]) ?? []);

/** Both Access settings present and valid: the deployed Worker can check sign-ins. */
export const accessReady = (s) => !!s.TEAM_DOMAIN && !!s.POLICY_AUD && !problem('TEAM_DOMAIN', s.TEAM_DOMAIN) && !problem('POLICY_AUD', s.POLICY_AUD);

/** `wrangler.jsonc` (already parsed) + settings -> the config to deploy. Pure. */
export function buildConfig(base, s) {
  const c = structuredClone(base);
  if (s.WORKER_NAME) c.name = s.WORKER_NAME;
  if (s.R2_BUCKET) {
    const b = (c.r2_buckets ?? []).find((x) => x.binding === 'TASKS');
    if (b) b.bucket_name = s.R2_BUCKET;
    else c.r2_buckets = [...(c.r2_buckets ?? []), { binding: 'TASKS', bucket_name: s.R2_BUCKET }];
  }
  c.vars = { ...(c.vars ?? {}) };
  if (s.TEAM_DOMAIN) c.vars.TEAM_DOMAIN = s.TEAM_DOMAIN;
  if (s.POLICY_AUD) c.vars.POLICY_AUD = s.POLICY_AUD;
  if (s.CUSTOM_DOMAIN) {
    c.routes = [{ pattern: s.CUSTOM_DOMAIN, custom_domain: true }];
    // The Access application covers the custom hostname; leaving workers.dev on would be a second,
    // unprotected front door to the static files (the API still re-checks the sign-in either way).
    c.workers_dev = false;
    c.preview_urls = false;
  }
  return c;
}

export function readBaseConfig() {
  return JSON.parse(stripJsonc(readFileSync(BASE_CONFIG, 'utf8')));
}

export function writeDeployConfig(s) {
  const cfg = buildConfig(readBaseConfig(), s);
  writeFileSync(DEPLOY_CONFIG, JSON.stringify(cfg, null, 2) + '\n');
  return cfg;
}

/** Run a command; by default its output goes straight to the terminal. */
export function run(cmd, args, opts = {}) {
  return spawnSync(cmd, args, { cwd: ROOT, stdio: 'inherit', ...opts });
}

/** Run quietly and return { ok, out }. stdin is closed so tools never stop to ask a question. */
export function capture(cmd, args) {
  const r = spawnSync(cmd, args, { cwd: ROOT, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
  return { ok: r.status === 0, out: `${r.stdout ?? ''}${r.stderr ?? ''}` };
}

export const has = (cmd, args = ['--version']) => capture(cmd, args).ok;

/** What is missing before the Worker can be built. Entries with `fix` can be installed for the user. */
export function missingTools() {
  if (!has('cargo')) return [{ what: 'Rust (cargo)', fix: null, hint: 'install it from https://rustup.rs, then run this again' }];
  const missing = [];
  if (!capture('rustup', ['target', 'list', '--installed']).out.includes('wasm32-unknown-unknown'))
    missing.push({ what: 'the wasm32 Rust target', fix: ['rustup', ['target', 'add', 'wasm32-unknown-unknown']] });
  if (!has('worker-build')) missing.push({ what: 'worker-build', fix: ['cargo', ['install', 'worker-build']] });
  return missing;
}

/** Deploy with the generated config (its build command builds the web app and the Worker). */
export function deploy(s) {
  writeDeployConfig(s);
  const r = spawnSync('npx', ['wrangler', 'deploy', '--config', DEPLOY_CONFIG], {
    cwd: ROOT, encoding: 'utf8', stdio: ['inherit', 'pipe', 'inherit'],
  });
  process.stdout.write(r.stdout ?? '');
  return { ok: r.status === 0, out: r.stdout ?? '' };
}

export const workerUrl = (out) => /https:\/\/[^\s]+\.workers\.dev/.exec(out)?.[0] ?? null;
