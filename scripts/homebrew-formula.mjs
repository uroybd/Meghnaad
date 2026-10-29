// The Homebrew formula for `meghnaad-hook` (the Lua hooks script as hooks for the real `task`), made from a release's builds: the
// Release workflow writes it into a tap (a repository `homebrew-<name>`) when a tag is released. Each platform's archive is the
// `meghnaad-hook-<version>-<target>.tar.gz` of the `hook` job; the formula names it by URL and checks its sha256.
//
//   node scripts/homebrew-formula.mjs --repo owner/name --version 1.2.3 DIR     # the formula, on standard output
//
// DIR holds the four archives. The archive has a folder in it; Homebrew steps into a lone folder by itself.
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

/** The platforms the program is built for, as the formula tells them apart. */
const PLATFORMS = [
  { os: 'macos', cpu: 'arm', target: 'aarch64-apple-darwin' },
  { os: 'macos', cpu: 'intel', target: 'x86_64-apple-darwin' },
  { os: 'linux', cpu: 'intel', target: 'x86_64-unknown-linux-gnu' },
  { os: 'linux', cpu: 'arm', target: 'aarch64-unknown-linux-gnu' },
];

/** The formula for `version` of the program released from `repo` (`owner/name`); `sums` is the sha256 of each target's archive. */
export function formula({ repo, version, sums }) {
  if (!/^[\w.-]+\/[\w.-]+$/.test(repo)) throw new Error(`"${repo}" is not a repository like owner/name`);
  if (!/^\d+\.\d+\.\d+$/.test(version)) throw new Error(`"${version}" is not a release version like 1.2.3`);
  const block = (os) =>
    PLATFORMS.filter((p) => p.os === os)
      .map(({ cpu, target }) => {
        const sum = sums[target];
        if (!/^[0-9a-f]{64}$/.test(sum ?? '')) throw new Error(`there is no sha256 for ${target}`);
        const url = `https://github.com/${repo}/releases/download/v${version}/meghnaad-hook-${version}-${target}.tar.gz`;
        return `    if Hardware::CPU.${cpu}?\n      url "${url}"\n      sha256 "${sum}"\n    end`;
      })
      .join('\n');
  return `class MeghnaadHook < Formula
  desc "Your Meghnaad Lua hooks script, as hooks for the real Taskwarrior task"
  homepage "https://github.com/${repo}"
  version "${version}"
  license "MIT"

  on_macos do
${block('macos')}
  end

  on_linux do
${block('linux')}
  end

  def install
    bin.install "meghnaad-hook"
  end

  def caveats
    <<~EOS
      Point task at your script, once (and again when it defines a hook it did not before):
        meghnaad-hook check ~/hooks.lua
        meghnaad-hook install ~/hooks.lua
      How it works: https://github.com/${repo}/blob/v${version}/docs/using/hooks.md#the-same-script-with-the-real-task
    EOS
  end

  test do
    (testpath/"hooks.lua").write "function on_add(task) end"
    assert_match "defines on_add", shell_output("#{bin}/meghnaad-hook check #{testpath}/hooks.lua")
  end
end
`;
}

/** The sha256 of each target's archive in `dir`. */
export function sumsIn(dir, version) {
  return Object.fromEntries(
    PLATFORMS.map(({ target }) => [
      target,
      createHash('sha256')
        .update(readFileSync(join(dir, `meghnaad-hook-${version}-${target}.tar.gz`)))
        .digest('hex'),
    ]),
  );
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const args = process.argv.slice(2);
  const take = (name) => {
    const at = args.indexOf(name);
    if (at < 0) throw new Error(`${name} is needed`);
    return args.splice(at, 2)[1];
  };
  const [repo, version] = [take('--repo'), take('--version')];
  const [dir] = args;
  if (!dir) throw new Error('give the folder the archives are in');
  process.stdout.write(formula({ repo, version, sums: sumsIn(dir, version) }));
}
