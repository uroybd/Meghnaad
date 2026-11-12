// The Homebrew formula for one of the programs a release builds for the desktop, `meghnaad-hook` (the Lua hooks script as
// hooks for the real `task`) or `meghnaad-tui` (the terminal app around the real `task`), made from the release's builds:
// the Release workflow writes both into a tap (a repository `homebrew-<name>`) when a tag is released. Each platform's
// archive is the `<program>-<version>-<target>.tar.gz` of the `programs` job; the formula names it by URL and checks its
// sha256.
//
//   node scripts/homebrew-formula.mjs --repo owner/name --version 1.2.3 [--program meghnaad-tui] DIR   # on standard output
//
// DIR holds the program's four archives; `--program` is `meghnaad-hook` when left out. The archive has a folder in it;
// Homebrew steps into a lone folder by itself.
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

/** What each program's formula says beyond its archives. */
export const PROGRAMS = {
  'meghnaad-hook': {
    className: 'MeghnaadHook',
    desc: 'Your Meghnaad Lua hooks script, as hooks for the real Taskwarrior task',
    depends: [],
    caveats: (
      repo,
      version,
    ) => `      Point task at your script, once (and again when it defines a hook it did not before):
        meghnaad-hook check ~/hooks.lua
        meghnaad-hook install ~/hooks.lua
      How it works: https://github.com/${repo}/blob/v${version}/docs/using/hooks.md#the-same-script-with-the-real-task`,
    test: `    (testpath/"hooks.lua").write "function on_add(task) end"
    assert_match "defines on_add", shell_output("#{bin}/meghnaad-hook check #{testpath}/hooks.lua")`,
  },
  'meghnaad-tui': {
    className: 'MeghnaadTui',
    desc: 'Meghnaad in the terminal: a TUI around the real Taskwarrior task',
    depends: ['task'],
    caveats: (repo, version) => `      Run meghnaad-tui: it works on the tasks task reads. Its task commands are kept in
      settings.json in $MEGHNAAD_CONFIG_DIR, else ~/.config/meghnaad.
      How it works: https://github.com/${repo}/blob/v${version}/docs/terminal.md`,
    test: `    assert_match "Taskwarrior", shell_output("#{bin}/meghnaad-tui --help")`,
  },
};

/** The formula for `version` of `program` released from `repo` (`owner/name`); `sums` is the sha256 of each target's archive. */
export function formula({ repo, version, sums, program = 'meghnaad-hook' }) {
  const p = PROGRAMS[program];
  if (!p) throw new Error(`"${program}" is not one of ${Object.keys(PROGRAMS).join(', ')}`);
  if (!/^[\w.-]+\/[\w.-]+$/.test(repo)) throw new Error(`"${repo}" is not a repository like owner/name`);
  if (!/^\d+\.\d+\.\d+$/.test(version)) throw new Error(`"${version}" is not a release version like 1.2.3`);
  const block = (os) =>
    PLATFORMS.filter((p) => p.os === os)
      .map(({ cpu, target }) => {
        const sum = sums[target];
        if (!/^[0-9a-f]{64}$/.test(sum ?? '')) throw new Error(`there is no sha256 for ${target}`);
        const url = `https://github.com/${repo}/releases/download/v${version}/${program}-${version}-${target}.tar.gz`;
        return `    if Hardware::CPU.${cpu}?\n      url "${url}"\n      sha256 "${sum}"\n    end`;
      })
      .join('\n');
  const depends = p.depends.map((d) => `  depends_on "${d}"\n`).join('');
  return `class ${p.className} < Formula
  desc "${p.desc}"
  homepage "https://github.com/${repo}"
  version "${version}"
  license "MIT"
${depends ? `\n${depends}` : ''}
  on_macos do
${block('macos')}
  end

  on_linux do
${block('linux')}
  end

  def install
    bin.install "${program}"
  end

  def caveats
    <<~EOS
${p.caveats(repo, version)}
    EOS
  end

  test do
${p.test}
  end
end
`;
}

/** The sha256 of each target's archive of `program` in `dir`. */
export function sumsIn(dir, version, program = 'meghnaad-hook') {
  return Object.fromEntries(
    PLATFORMS.map(({ target }) => [
      target,
      createHash('sha256')
        .update(readFileSync(join(dir, `${program}-${version}-${target}.tar.gz`)))
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
  const program = args.includes('--program') ? take('--program') : 'meghnaad-hook';
  const [dir] = args;
  if (!dir) throw new Error('give the folder the archives are in');
  process.stdout.write(formula({ repo, version, program, sums: sumsIn(dir, version, program) }));
}
