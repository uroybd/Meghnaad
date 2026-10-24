// @vitest-environment jsdom
import { afterEach, beforeAll, describe, expect, it } from 'vitest';
import { createRcEditor, type RcEditor } from './rcEditor';

// jsdom has no layout: CodeMirror measures with these, so they answer with nothing.
beforeAll(() => {
  const none = () => ({ top: 0, left: 0, right: 0, bottom: 0, width: 0, height: 0, x: 0, y: 0, toJSON: () => ({}) });
  Range.prototype.getBoundingClientRect = none;
  Range.prototype.getClientRects = () => [] as unknown as DOMRectList;
});

let editor: RcEditor | null = null;
afterEach(() => {
  editor?.destroy();
  editor = null;
  document.body.innerHTML = '';
});

function open(onChange: (t: string) => void = () => {}) {
  editor = createRcEditor(document.body, onChange);
  return editor;
}
const spans = (kind: string) => [...document.querySelectorAll(`.cm-rc-${kind}`)].map((e) => e.textContent);

describe('the taskrc editor', () => {
  it('numbers the lines and colours what each one is', () => {
    open().show('taskrc', '# mine\nbulk=3 # three\ninclude holidays/en.rc\nnot a setting\n', 'empty');
    expect([...document.querySelectorAll('.cm-lineNumbers .cm-gutterElement')].map((e) => e.textContent)).toContain(
      '4',
    );
    expect(spans('comment')).toEqual(['# mine', '# three']);
    expect(spans('key')).toEqual(['bulk']);
    expect(spans('value')).toEqual(['3']);
    expect(spans('keyword')).toEqual(['include']);
    expect(spans('path')).toEqual(['holidays/en.rc']);
    expect(spans('bad')).toEqual(['not a setting']);
  });

  it('colours again as it is typed in, and tells what is in it', () => {
    const seen: string[] = [];
    const e = open((t) => seen.push(t));
    e.show('taskrc', 'bulk=3\n', '');
    e.show('taskrc', 'bulk=3\nsync.encryption_secret=x\n', '');
    expect(seen.at(-1)).toBe('bulk=3\nsync.encryption_secret=x\n');
    expect(e.text()).toBe('bulk=3\nsync.encryption_secret=x\n');
    expect(spans('secret')).toEqual(['sync.encryption_secret']);
  });

  it('starts another file afresh, so undo does not reach back into the last one', () => {
    const e = open();
    e.show('a.rc', 'x=1\n', '');
    e.show('b.rc', 'y=2\n', '');
    expect(e.text()).toBe('y=2\n');
    expect(document.querySelector('.cm-content')?.getAttribute('aria-label')).toBe('Text of b.rc');
    expect(spans('key')).toEqual(['y']);
  });

  it('marks a line the engine has something to say about, and can be locked while a file loads', () => {
    const e = open();
    e.show('taskrc', 'a=1\ninclude missing.rc\n', '');
    e.mark([
      { line: 2, message: 'no such file' },
      { line: 9, message: 'past the end' },
    ]);
    e.setEditable(false);
    expect(document.querySelector('.cm-content')?.getAttribute('contenteditable')).toBe('false');
    e.setEditable(true);
    expect(document.querySelector('.cm-content')?.getAttribute('contenteditable')).toBe('true');
  });
});
