#!/usr/bin/env node
// `npm run deploy`: deploy with the choices saved in `.deploy.vars` (see `npm run setup`).
//   npm run deploy -- --check    show the config that would be deployed, without deploying
import { accessReady, deploy, problems, readSettings, readBaseConfig, buildConfig } from './deploy-lib.mjs';

const s = readSettings();
const bad = problems(s);
if (bad.length) {
  console.error('Invalid settings:\n  ' + bad.join('\n  '));
  process.exit(1);
}

if (process.argv.includes('--check')) {
  console.log(JSON.stringify(buildConfig(readBaseConfig(), s), null, 2));
  process.exit(0);
}

if (!accessReady(s)) {
  console.warn(
    'Note: TEAM_DOMAIN / POLICY_AUD are not set, so the deployed app will refuse every request and show a\n' +
      'setup page. Run `npm run setup` to connect Cloudflare Access.\n',
  );
}
process.exit(deploy(s).ok ? 0 : 1);
