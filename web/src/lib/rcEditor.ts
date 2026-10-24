// The editor for the taskrc files: CodeMirror with line numbers, colour for what each line is, and markers for the lines the
// engine has something to say about. What a piece of a line is (a setting's name, an `include`, a comment, a line the app will
// comment out) is the engine's (`tc_core::rc_highlight`, through src/engine/helpers.ts), the same on every front end. It is its
// own module so that the taskrc tab loads it (with the hooks editor's chunk) only when it is opened.

import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands';
import { lintGutter, linter, setDiagnostics, type Diagnostic } from '@codemirror/lint';
import { Compartment, EditorState, StateField, type Extension } from '@codemirror/state';
import {
  Decoration,
  drawSelection,
  EditorView,
  highlightActiveLine,
  keymap,
  lineNumbers,
  placeholder,
  type DecorationSet,
} from '@codemirror/view';
import { callHelper } from '../engine/helpers';
import { editorTheme } from './editorTheme';

export interface RcEditor {
  text(): string;
  /**
   * Show `text` as the file `name`: another file starts afresh (no undo into the last one), the same one is an edit. `hint` is
   * what an empty file says.
   */
  show(name: string, text: string, hint: string): void;
  /** Mark the lines the engine has something to say about. */
  mark(problems: { line: number; message: string }[]): void;
  /** Whether it can be typed in (not while a file loads). */
  setEditable(on: boolean): void;
  focus(): void;
  destroy(): void;
}

type Kind = 'comment' | 'marker' | 'keyword' | 'path' | 'key' | 'punct' | 'value' | 'bad' | 'secret';
interface Piece {
  start: number;
  end: number;
  kind: Kind;
}

const KINDS: Kind[] = ['comment', 'marker', 'keyword', 'path', 'key', 'punct', 'value', 'bad', 'secret'];
const marks = Object.fromEntries(KINDS.map((k) => [k, Decoration.mark({ class: `cm-rc-${k}` })])) as Record<
  Kind,
  Decoration
>;

function coloured(text: string): DecorationSet {
  const pieces = callHelper<Piece[]>('taskrc_highlight', text);
  return Decoration.set(
    pieces.filter((p) => p.end <= text.length).map((p) => marks[p.kind].range(p.start, p.end)),
    true,
  );
}

const colours = StateField.define<DecorationSet>({
  create: (state) => coloured(state.doc.toString()),
  update: (value, tr) => (tr.docChanged ? coloured(tr.state.doc.toString()) : value),
  provide: (f) => EditorView.decorations.from(f),
});

// The colours of the pieces come from the page's variables, as the console's line does.
const look = EditorView.theme({
  '.cm-rc-comment': { color: 'var(--dim)', fontStyle: 'italic' },
  '.cm-rc-marker': { color: 'var(--warn)', fontStyle: 'normal', fontWeight: '600' },
  '.cm-rc-keyword': { color: 'var(--accent)', fontWeight: '600' },
  '.cm-rc-path': { color: 'var(--hl-str)' },
  '.cm-rc-key': { color: 'var(--hl-attr)' },
  '.cm-rc-punct': { color: 'var(--dim)' },
  '.cm-rc-value': { color: 'var(--text)' },
  '.cm-rc-bad': { textDecoration: 'underline wavy var(--warn)' },
  '.cm-rc-secret': { color: 'var(--err)', textDecoration: 'line-through' },
});

export function createRcEditor(parent: HTMLElement, onChange: (text: string) => void): RcEditor {
  const editable = new Compartment();
  const label = new Compartment();
  const extensions = (name: string, hint: string): Extension[] => [
    lineNumbers(),
    EditorView.lineWrapping,
    history(),
    drawSelection(),
    highlightActiveLine(),
    colours,
    // The panel says what is wrong (the engine's own reading of the folder); this only holds what it finds.
    linter(() => []),
    lintGutter(),
    keymap.of([...defaultKeymap, ...historyKeymap, indentWithTab]),
    editorTheme,
    look,
    placeholder(hint),
    editable.of(EditorView.editable.of(true)),
    label.of(EditorView.contentAttributes.of({ 'aria-label': `Text of ${name}`, spellcheck: 'false' })),
    EditorView.updateListener.of((u) => {
      if (u.docChanged) onChange(u.state.doc.toString());
    }),
  ];
  const view = new EditorView({
    parent,
    state: EditorState.create({ doc: '', extensions: extensions('the file', '') }),
  });
  let shown = '';
  let on = true;
  return {
    text: () => view.state.doc.toString(),
    show(name, text, hint) {
      if (name !== shown) {
        shown = name;
        view.setState(EditorState.create({ doc: text, extensions: extensions(name, hint) }));
        view.dispatch({ effects: editable.reconfigure(EditorView.editable.of(on)) });
      } else if (view.state.doc.toString() !== text) {
        view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text } });
      }
    },
    mark(problems) {
      const doc = view.state.doc;
      const diagnostics: Diagnostic[] = problems.flatMap((p) => {
        if (p.line < 1 || p.line > doc.lines) return [];
        const line = doc.line(p.line);
        return [
          { from: line.from, to: Math.max(line.to, line.from + 1), severity: 'warning' as const, message: p.message },
        ];
      });
      view.dispatch(setDiagnostics(view.state, diagnostics));
    },
    setEditable(next) {
      on = next;
      view.dispatch({ effects: editable.reconfigure(EditorView.editable.of(next)) });
    },
    focus: () => view.focus(),
    destroy: () => view.destroy(),
  };
}
