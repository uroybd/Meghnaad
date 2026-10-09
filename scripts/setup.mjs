#!/usr/bin/env node
// `npm run setup`: a guided, re-runnable first deployment.
//   1 build tools · 2 Cloudflare login · 3 your choices (bucket, custom domain, name) · 4 deploy
//   5 encryption secret · 6 Cloudflare Access, then a final deploy that locks the app to you.
// Every step checks first and skips what is already done, so running it again is safe.
import { createInterface } from 'node:readline/promises';
import {
  accessReady,
  capture,
  DEPLOY_CONFIG,
  deploy,
  missingTools,
  problem,
  readBaseConfig,
  readSettings,
  run,
  workerUrl,
  writeSettings,
} from './deploy-lib.mjs';

const rl = createInterface({ input: process.stdin, output: process.stdout });
const ask = async (q, def = '') => (await rl.question(def ? `${q} [${def}] ` : `${q} `)).trim() || def;
const yes = async (q, def = true) => /^y/i.test(await ask(`${q} (${def ? 'Y/n' : 'y/N'})`, def ? 'y' : 'n'));
const step = (n, t) => console.log(`\n\x1b[1m${n}. ${t}\x1b[0m`);
const done = (t) => console.log(`   ✓ ${t}`);
const die = (t) => {
  console.error(`\n✗ ${t}`);
  rl.close();
  process.exit(1);
};
const wrangler = (...a) => capture('npx', ['wrangler', ...a]);

/** Ask until the answer is valid (or empty, when allowed). */
async function askValid(key, q, def = '', required = false) {
  for (;;) {
    const v = (await ask(q, def)).replace(/\/$/, '');
    const p = problem(key, v);
    if (!p && (v || !required)) return v;
    console.log(`   ${p ?? 'This one is required.'}`);
  }
}

let s = readSettings();
const base = readBaseConfig();

// ---- 1
step(1, 'Build tools');
for (const m of missingTools()) {
  if (!m.fix) die(`${m.what} is missing: ${m.hint}`);
  if (!(await yes(`${m.what} is missing. Install it now (${m.fix[0]} ${m.fix[1].join(' ')})?`)))
    die('Cannot continue without it.');
  if (run(...m.fix).status !== 0) die(`Installing ${m.what} failed.`);
}
done('Rust, the wasm32 target and worker-build are ready');

// ---- 2
step(2, 'Cloudflare account');
if (!/You are logged in/i.test(wrangler('whoami').out)) {
  console.log('   Opening the browser to log in…');
  if (run('npx', ['wrangler', 'login']).status !== 0) die('Login failed.');
}
done('logged in');

// ---- 3
step(3, 'Your choices');
const defaultBucket = base.r2_buckets?.find((b) => b.binding === 'TASKS')?.bucket_name ?? 'taskwarrior-sync';
console.log(
  '   R2 bucket: the one your `task` CLI already syncs to (an existing bucket is used as is; a new name is created).',
);
s.R2_BUCKET = await askValid('R2_BUCKET', '   Bucket name:', s.R2_BUCKET || defaultBucket, true);
console.log(
  '   Custom domain: optional, e.g. tasks.example.com. The domain must already be on your Cloudflare account.',
);
console.log('   Leave empty to use the free *.workers.dev address.');
s.CUSTOM_DOMAIN = await askValid('CUSTOM_DOMAIN', '   Custom domain:', s.CUSTOM_DOMAIN);
console.log(`   Worker name: optional, only to run a second copy (default "${base.name}").`);
const name = await askValid('WORKER_NAME', '   Worker name:', s.WORKER_NAME || base.name);
s.WORKER_NAME = name === base.name ? '' : name;
writeSettings(s);

const names = [...wrangler('r2', 'bucket', 'list').out.matchAll(/^name:\s+(\S+)/gm)].map((m) => m[1]);
if (names.includes(s.R2_BUCKET)) done(`using the existing bucket "${s.R2_BUCKET}"`);
else {
  // stdin is closed inside capture(), so wrangler doesn't offer to add a stray binding to wrangler.jsonc.
  const r = wrangler('r2', 'bucket', 'create', s.R2_BUCKET);
  if (!r.ok) die(`Could not create the bucket:\n${r.out}`);
  done(`created "${s.R2_BUCKET}"`);
}
done('saved to .deploy.vars (git-ignored)');

// ---- 4
step(4, 'Deploying the Worker');
const first = deploy(s);
if (!first.ok) die('Deploy failed (see the output above).');
const url = s.CUSTOM_DOMAIN ? `https://${s.CUSTOM_DOMAIN}` : workerUrl(first.out);
done(url ? `live at ${url}` : 'deployed');
if (!accessReady(s))
  console.log(
    '   Until Access is connected (step 6) every request is refused and a setup page is shown. That is intentional.',
  );

// ---- 5
step(5, 'Encryption secret');
const have = wrangler('secret', 'list', '--format', 'json', '--config', DEPLOY_CONFIG).out.includes(
  'TC_ENCRYPTION_SECRET',
);
if (have && !(await yes('A secret is already set. Replace it?', false))) done('keeping the existing secret');
else {
  console.log('   Enter the SAME value as `sync.encryption_secret` in your taskrc (it is not echoed).');
  if (run('npx', ['wrangler', 'secret', 'put', 'TC_ENCRYPTION_SECRET', '--config', DEPLOY_CONFIG]).status !== 0)
    die('Setting the secret failed.');
  done('secret stored in Cloudflare');
}

// ---- 6
step(6, 'Cloudflare Access (who may open the app)');
if (accessReady(s) && !(await yes('Access settings are already saved. Change them?', false)))
  done('keeping the saved settings');
else {
  const host = url ? new URL(url).host : '<your-worker>.workers.dev';
  console.log(`
   In the Cloudflare dashboard:
     Access controls -> Applications -> Add an application -> Self-hosted
       Destination: ${host}
       Policy: Allow, selector "Emails", your email address
       (Login methods: One-time PIN works out of the box.)
     Then copy the Application Audience (AUD) tag from the application's page.
     Your team domain (https://<team>.cloudflareaccess.com) is under Access controls -> Settings.
   Press Enter at a prompt to skip and finish later with \`npm run setup\`.`);
  const team = await askValid('TEAM_DOMAIN', '   Team domain:', s.TEAM_DOMAIN);
  const aud = team ? await askValid('POLICY_AUD', '   AUD tag:', s.POLICY_AUD) : '';
  if (!team || !aud) {
    console.log('\n   Skipped. The app stays locked (it shows a setup page) until you run `npm run setup` again.');
    rl.close();
    process.exit(0);
  }
  s.TEAM_DOMAIN = team;
  s.POLICY_AUD = aud;
  writeSettings(s);
  done('saved to .deploy.vars');
  step('6b', 'Deploying with Access enabled');
  if (!deploy(s).ok) die('Deploy failed.');
}

console.log(`\n\x1b[1mDone.\x1b[0m Open ${url ?? 'your Worker URL'}, sign in, then import your taskrc under "taskrc".`);
console.log('Next time: `npm run deploy`.');
rl.close();
