// The editor for the hooks script: CodeMirror with Lua highlighting and error markers. It is its own module so that
// the Hooks tab loads it (about 150 KB) only when it is opened, and nothing else pays for it.

import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands';
import {
  bracketMatching,
  HighlightStyle,
  indentOnInput,
  StreamLanguage,
  syntaxHighlighting,
} from '@codemirror/language';
import { lua } from '@codemirror/legacy-modes/mode/lua';
import { linter, lintGutter, setDiagnostics, type Diagnostic } from '@codemirror/lint';
import { EditorState } from '@codemirror/state';
import { drawSelection, EditorView, highlightActiveLine, keymap, lineNumbers } from '@codemirror/view';
import { editorTheme as theme } from './editorTheme';
import { tags } from '@lezer/highlight';

export interface HookEditor {
  text(): string;
  setText(text: string): void;
  /** Mark what is wrong: a line of the script, or none (then it is shown by the panel, not in the text). */
  mark(problems: { line: number | null; message: string }[]): void;
  focus(): void;
  destroy(): void;
}

const highlight = HighlightStyle.define([
  { tag: tags.keyword, color: 'var(--accent)', fontWeight: '600' },
  { tag: [tags.string, tags.special(tags.string)], color: 'var(--ok)' },
  { tag: tags.number, color: 'var(--warn)' },
  { tag: tags.comment, color: 'var(--dim)', fontStyle: 'italic' },
  { tag: [tags.function(tags.variableName), tags.definition(tags.variableName)], color: 'var(--accent)' },
  { tag: [tags.bool, tags.null, tags.atom], color: 'var(--warn)' },
  { tag: tags.operator, color: 'var(--text)' },
]);

export function createHookEditor(parent: HTMLElement, initial: string, onChange: (text: string) => void): HookEditor {
  const view = new EditorView({
    parent,
    state: EditorState.create({
      doc: initial,
      extensions: [
        lineNumbers(),
        history(),
        drawSelection(),
        highlightActiveLine(),
        indentOnInput(),
        bracketMatching(),
        StreamLanguage.define(lua),
        syntaxHighlighting(highlight),
        // The panel does the checking (the engine's own Lua parser, on the server); this only holds what it finds.
        linter(() => []),
        lintGutter(),
        keymap.of([...defaultKeymap, ...historyKeymap, indentWithTab]),
        theme,
        EditorView.contentAttributes.of({ 'aria-label': 'Hooks script (Lua)', spellcheck: 'false' }),
        EditorView.updateListener.of((u) => {
          if (u.docChanged) onChange(u.state.doc.toString());
        }),
      ],
    }),
  });
  return {
    text: () => view.state.doc.toString(),
    setText(text) {
      view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text } });
    },
    mark(problems) {
      const doc = view.state.doc;
      const diagnostics: Diagnostic[] = problems.flatMap((p) => {
        if (p.line === null || p.line < 1 || p.line > doc.lines) return [];
        const line = doc.line(p.line);
        return [
          { from: line.from, to: Math.max(line.to, line.from + 1), severity: 'error' as const, message: p.message },
        ];
      });
      view.dispatch(setDiagnostics(view.state, diagnostics));
    },
    focus: () => view.focus(),
    destroy: () => view.destroy(),
  };
}
