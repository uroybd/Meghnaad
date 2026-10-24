// The look both code editors (the hooks script and the taskrc files) share, so they follow the page's own variables and so
// its light and dark schemes. Part of the chunk that loads when an editor is opened.

import { EditorView } from '@codemirror/view';

export const editorTheme = EditorView.theme({
  '&': { color: 'var(--text)', backgroundColor: 'var(--panel)', border: '1px solid var(--line)', borderRadius: '6px' },
  '&.cm-focused': { outline: '2px solid var(--accent)' },
  '.cm-content': { fontFamily: 'var(--mono, ui-monospace, monospace)', fontSize: '13px', caretColor: 'var(--text)' },
  '.cm-gutters': { backgroundColor: 'transparent', color: 'var(--dim)', border: 'none' },
  '.cm-activeLine': { backgroundColor: 'color-mix(in srgb, var(--accent) 7%, transparent)' },
  '.cm-activeLineGutter': { backgroundColor: 'transparent' },
  '.cm-selectionBackground, &.cm-focused .cm-selectionBackground': {
    backgroundColor: 'color-mix(in srgb, var(--accent) 28%, transparent)',
  },
  '.cm-scroller': { maxHeight: '60vh', minHeight: '260px', overflow: 'auto' },
});
