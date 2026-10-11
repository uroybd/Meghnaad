#!/usr/bin/env node
// Builds the Android app: the web app, the engine in Rust for the phone's processors, then the APK.
//   node scripts/build-android.mjs            a debug APK (android/app/build/outputs/apk/debug)
//   node scripts/build-android.mjs --release  a release bundle's APK (unsigned unless a keystore is configured)
// It needs the Android SDK and NDK (Android Studio installs both) and `cargo-ndk`; a JDK 21 is taken from Android Studio
// when JAVA_HOME is not one.
import { existsSync, readdirSync, rmSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { ROOT, capture, has, run } from './deploy-lib.mjs';
import { currentVersion } from './version.mjs';

const release = process.argv.includes('--release');
const step = (m) => console.log(`\n[android] ${m}`);
const must = (r, what) => {
  if (r.status !== 0) {
    console.error(`[android] ${what} failed`);
    process.exit(r.status ?? 1);
  }
};
const die = (m) => {
  console.error(`[android] ${m}`);
  process.exit(1);
};

const sdk = process.env.ANDROID_HOME ?? process.env.ANDROID_SDK_ROOT ?? join(homedir(), 'Library', 'Android', 'sdk');
if (!existsSync(sdk)) die(`the Android SDK is not at ${sdk}: install Android Studio, or set ANDROID_HOME`);
const ndks = existsSync(join(sdk, 'ndk')) ? readdirSync(join(sdk, 'ndk')).sort() : [];
if (!ndks.length) die('the Android NDK is missing: install it from Android Studio (SDK Manager > SDK Tools > NDK)');
if (!has('cargo', ['ndk', '--version'])) die('cargo-ndk is missing: cargo install cargo-ndk');
const targets = ['aarch64-linux-android', 'x86_64-linux-android'];
const installed = capture('rustup', ['target', 'list', '--installed']).out;
const missing = targets.filter((t) => !installed.includes(t));
if (missing.length) must(run('rustup', ['target', 'add', ...missing]), 'installing the Rust targets');

const javaHome = (() => {
  const own = process.env.JAVA_HOME;
  if (own && /version "(2\d|[3-9]\d)/.test(capture(join(own, 'bin', 'java'), ['-version']).out)) return own;
  const studio = '/Applications/Android Studio.app/Contents/jbr/Contents/Home';
  return existsSync(studio) ? studio : own;
})();
if (!javaHome) die('a JDK 21 is needed: set JAVA_HOME, or install Android Studio');

const version = currentVersion();
const env = {
  ANDROID_VERSION_NAME: version.name,
  ANDROID_VERSION_CODE: String(version.code),
  ...process.env,
  ANDROID_HOME: sdk,
  ANDROID_NDK_HOME: join(sdk, 'ndk', ndks.at(-1)),
  JAVA_HOME: javaHome,
};

step('web app');
must(run('node', ['scripts/build-engine.mjs', '--if-stale']), 'the engine build');
must(run('npm', ['--prefix', 'web', 'run', 'build']), 'the web build');

step('engine for the phone (Rust)');
const libs = join(ROOT, 'android', 'app', 'src', 'main', 'jniLibs');
rmSync(libs, { recursive: true, force: true });
must(
  run('cargo', ['ndk', '-t', 'arm64-v8a', '-t', 'x86_64', '-o', libs, 'build', '-p', 'tc-android', '--release'], {
    env,
  }),
  'cargo ndk',
);
// cargo-ndk also copies every other dynamic library of the build (taskchampion's own); the app loads only ours.
for (const abi of readdirSync(libs))
  for (const f of readdirSync(join(libs, abi))) if (f !== 'libtc_android.so') rmSync(join(libs, abi, f));

step('copying the web app into the project');
must(run('npx', ['cap', 'sync', 'android'], { cwd: join(ROOT, 'web'), env }), 'cap sync');

step(release ? 'APK (release)' : 'APK (debug)');
must(run('./gradlew', [release ? 'assembleRelease' : 'assembleDebug'], { cwd: join(ROOT, 'android'), env }), 'gradle');
console.log(
  `\n[android] done: android/app/build/outputs/apk/${release ? 'release' : 'debug'}/ (install with: adb install -r <apk>)`,
);
