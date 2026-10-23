import { describe, expect, it } from 'vitest';
import { includeLine, listing, uploadFiles, withInclude } from './rcpaths';

describe('the include line that names a file from another', () => {
  it('is written from the folder of the file that holds it', () => {
    expect(includeLine('taskrc', 'holidays/en-US.rc')).toBe('include holidays/en-US.rc');
    expect(includeLine('holidays/en-US.rc', 'holidays/extra.rc')).toBe('include extra.rc');
    expect(includeLine('holidays/en-US.rc', 'taskrc')).toBe('include ../taskrc');
    expect(includeLine('a/b/c.rc', 'a/x/y.rc')).toBe('include ../x/y.rc');
    expect(includeLine('taskrc', 'my notes.rc')).toBe('include my notes.rc');
  });

  it('goes on a line of its own, once', () => {
    expect(withInclude('bulk=3', 'taskrc', 'a.rc')).toBe('bulk=3\ninclude a.rc\n');
    expect(withInclude('bulk=3\n', 'taskrc', 'a.rc')).toBe('bulk=3\ninclude a.rc\n');
    expect(withInclude('', 'taskrc', 'a.rc')).toBe('include a.rc\n');
    expect(withInclude('include a.rc\r\nbulk=3', 'taskrc', 'a.rc')).toBe('include a.rc\r\nbulk=3');
  });
});

describe('the files of an upload', () => {
  it('lose the folder that was picked, and .taskrc is taskrc', () => {
    const r = uploadFiles([
      { name: 'en-US.rc', relative: 'dots/holidays/en-US.rc', text: 'holiday.a.name=A\n' },
      { name: '.taskrc', relative: 'dots/.taskrc', text: 'bulk=3\n' },
    ]);
    expect(r.files.map((f) => f.path)).toEqual(['taskrc', 'holidays/en-US.rc']);
    expect(r.skipped).toEqual([]);
  });

  it('keep a plain file under its own name', () => {
    expect(uploadFiles([{ name: 'a.rc', text: 'x' }]).files).toEqual([{ path: 'a.rc', text: 'x' }]);
  });

  it('leave out hidden files, binary files, large ones and repeats, and say so', () => {
    const r = uploadFiles([
      { name: 'HEAD', relative: 'dots/.git/HEAD', text: 'ref' },
      { name: '.DS_Store', relative: 'dots/.DS_Store', text: 'x' },
      { name: 'logo.png', relative: 'dots/logo.png', text: 'a\u0000b' },
      { name: 'big.rc', relative: 'dots/big.rc', text: 'x'.repeat(300 * 1024) },
      { name: 'ok.rc', relative: 'dots/ok.rc', text: 'bulk=1\n' },
      { name: 'ok.rc', relative: 'dots/ok.rc', text: 'bulk=2\n' },
    ]);
    expect(r.files.map((f) => f.path)).toEqual(['ok.rc']);
    expect(r.skipped).toEqual([
      'dots/.git/HEAD (hidden)',
      'dots/.DS_Store (hidden)',
      'dots/logo.png (not text)',
      'dots/big.rc (too large)',
      'dots/ok.rc (the same name as another)',
    ]);
  });

  it('stop at the most a folder holds', () => {
    const many = Array.from({ length: 70 }, (_, i) => ({ name: `f${i}.rc`, text: 'bulk=1\n' }));
    const r = uploadFiles(many);
    expect(r.files).toHaveLength(64);
    expect(r.skipped).toHaveLength(6);
  });
});

describe('the list of files', () => {
  it('puts taskrc first and names each folder above its files', () => {
    expect(listing(['holidays/en-US.rc', 'taskrc', 'a.rc', 'holidays/de.rc'])).toEqual([
      { path: 'taskrc', depth: 0, name: 'taskrc', folder: null },
      { path: 'a.rc', depth: 0, name: 'a.rc', folder: null },
      { path: '', depth: 0, name: 'holidays', folder: 'holidays' },
      { path: 'holidays/de.rc', depth: 1, name: 'de.rc', folder: 'holidays' },
      { path: 'holidays/en-US.rc', depth: 1, name: 'en-US.rc', folder: 'holidays' },
    ]);
  });
});
