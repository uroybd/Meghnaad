#!/usr/bin/env node
// Builds the Android app (app/): the engine in Rust for the phone's processors, then the APK.
//   node scripts/build-android.mjs            a debug APK (app/androidApp/build/outputs/apk/debug)
//   node scripts/build-android.mjs --release  a release APK (unsigned unless a keystore is configured)
// It needs the Android SDK and NDK (Android Studio installs both) and `cargo-ndk`; a JDK 21 is taken from Android Studio
// when JAVA_HOME is not one.
import { existsSync, readdirSync, rmSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { ROOT, capture, has, run } from './deploy-lib.mjs';
import { currentVersion } from './version.mjs';

const release = process.argv.includes('--release');
// A release is for phones (arm64); a debug build also runs in the emulator on an Intel machine (x86_64).
const abis = (process.env.ANDROID_ABIS ?? (release ? 'arm64-v8a' : 'arm64-v8a,x86_64')).split(',');
const targetOf = { 'arm64-v8a': 'aarch64-linux-android', x86_64: 'x86_64-linux-android' };
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
const targets = abis.map((a) => targetOf[a]);
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
  ANDROID_ABIS: abis.join(','),
  ANDROID_VERSION_NAME: version.name,
  ANDROID_VERSION_CODE: String(version.code),
  ...process.env,
  ANDROID_HOME: sdk,
  ANDROID_NDK_HOME: join(sdk, 'ndk', ndks.at(-1)),
  JAVA_HOME: javaHome,
};

step('engine for the phone (Rust)');
const libs = join(ROOT, 'app', 'androidApp', 'src', 'main', 'jniLibs');
rmSync(libs, { recursive: true, force: true });
must(
  run(
    'cargo',
    ['ndk', ...abis.flatMap((a) => ['-t', a]), '-o', libs, 'build', '-p', 'tc-android', '--profile', 'android'],
    {
      env,
    },
  ),
  'cargo ndk',
);
// cargo-ndk also copies every other dynamic library of the build (taskchampion's own); the app loads only ours.
for (const abi of readdirSync(libs))
  for (const f of readdirSync(join(libs, abi))) if (f !== 'libtc_android.so') rmSync(join(libs, abi, f));

step(release ? 'APK (release)' : 'APK (debug)');
must(
  run('./gradlew', [release ? ':androidApp:assembleRelease' : ':androidApp:assembleDebug'], {
    cwd: join(ROOT, 'app'),
    env,
  }),
  'gradle',
);
console.log(
  `\n[android] done: app/androidApp/build/outputs/apk/${release ? 'release' : 'debug'}/ (install with: adb install -r <apk>)`,
);
