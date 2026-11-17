// @vitest-environment node
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { open, seal, WrongPassword } from './pgp';

const text = (b: Uint8Array) => new TextDecoder().decode(b);
const bytes = (s: string) => new TextEncoder().encode(s);

const hasGpg = spawnSync('gpg', ['--version']).status === 0;

describe('a message sealed with a password', () => {
  it('opens with the same password, whatever its size', async () => {
    for (const size of [0, 1, 15, 16, 17, 300, 9000]) {
      const data = new Uint8Array(size).map((_, i) => (i * 7) & 0xff);
      expect(await open(await seal(data, 'correct horse'), 'correct horse')).toEqual(data);
    }
  });

  it('refuses another password, and a changed message', async () => {
    const sealed = await seal(bytes('{"token":"ghp_x"}'), 'right');
    await expect(open(sealed, 'wrong')).rejects.toBeInstanceOf(WrongPassword);
    const changed = sealed.slice();
    changed[changed.length - 5] ^= 1;
    await expect(open(changed, 'right')).rejects.toBeInstanceOf(WrongPassword);
    await expect(open(bytes('not a message'), 'right')).rejects.toBeInstanceOf(WrongPassword);
  });

  it('holds nothing of the data or the password in the clear', async () => {
    const sealed = text(await seal(bytes('ghp_secrettoken'), 'my secret'));
    expect(sealed).not.toContain('ghp_secrettoken');
    expect(sealed).not.toContain('my secret');
  });

  it.skipIf(!hasGpg)('is opened by gpg --decrypt', async () => {
    const home = mkdtempSync(join(tmpdir(), 'meghnaad-gpg-'));
    try {
      const file = join(home, 'sealed.gpg');
      writeFileSync(file, await seal(bytes('{"repo":"alice/tasks"}'), 'pässword'));
      const out = execFileSync(
        'gpg',
        [
          '--homedir',
          home,
          '--batch',
          '--quiet',
          '--pinentry-mode',
          'loopback',
          '--passphrase',
          'pässword',
          '--decrypt',
          file,
        ],
        { stdio: ['ignore', 'pipe', 'ignore'] },
      );
      expect(out.toString()).toBe('{"repo":"alice/tasks"}');
      // And one gpg sealed the same way (`--symmetric`, SEIPD with MDC, AES-256) opens here.
      const plain = join(home, 'plain.txt');
      writeFileSync(plain, 'from gpg');
      execFileSync(
        'gpg',
        [
          '--homedir',
          home,
          '--batch',
          '--quiet',
          '--pinentry-mode',
          'loopback',
          '--passphrase',
          'pw',
          '--symmetric',
          '--cipher-algo',
          'AES256',
          '--s2k-digest-algo',
          'SHA256',
          '--s2k-mode',
          '3',
          '--compress-algo',
          'none',
          '--rfc4880',
          '-o',
          join(home, 'from.gpg'),
          plain,
        ],
        { stdio: 'ignore' },
      );
      expect(text(await open(new Uint8Array(readFileSync(join(home, 'from.gpg'))), 'pw'))).toBe('from gpg');
    } finally {
      rmSync(home, { recursive: true, force: true });
    }
  });
});
