<script lang="ts">
  import { untrack } from 'svelte';
  import { debounce } from './debounce';
  import type { RcEditor } from './rcEditor';

  let {
    name,
    text,
    hint,
    loading,
    ready,
    problems,
    ontext,
  }: {
    /** The file shown. */
    name: string;
    /** Its text, as the panel's draft has it. */
    text: string;
    /** What an empty file says. */
    hint: string;
    /** The file is being read. */
    loading: boolean;
    /** Its text is there to show (a draft of it exists). */
    ready: boolean;
    /** The lines of this file with a problem, marked in the margin. */
    problems: { line: number; message: string }[];
    /** What is in the box, once typing stops. */
    ontext: (text: string) => void;
  } = $props();

  // The text is edited in CodeMirror (line numbers, colour for what each line is, markers for lines with a problem), which loads
  // with this tab. The panel's drafts stay the one place the text is kept: the editor shows the file shown, and what is typed
  // reaches the draft once typing stops (`typed`), read from the editor then, so nothing typed meanwhile is lost.
  let host: HTMLElement | undefined = $state();
  let editor = $state.raw<RcEditor>();
  const typed = debounce(() => {
    if (editor) ontext(editor.text());
  }, 150);

  /** Hand over what was typed now, before the draft is read (a save, another file). */
  export function flush() {
    typed.flush();
  }
  /** Forget what was typed and not yet handed over (the edits are being reverted). */
  export function cancel() {
    typed.cancel();
  }

  $effect(() => {
    if (!host) return;
    let alive = true;
    const parent = host;
    void import('./rcEditor').then(({ createRcEditor }) => {
      if (alive) editor = createRcEditor(parent, () => typed());
    });
    return () => {
      alive = false;
      typed.cancel();
      editor?.destroy();
      editor = undefined;
    };
  });
  $effect(() => {
    if (!editor || !ready) return;
    const [file, now, empty] = [name, text, hint];
    untrack(() => editor?.show(file, now, empty));
  });
  $effect(() => {
    editor?.setEditable(!loading && ready);
  });
  // The markers in the editor follow the problems; drawn again whenever another file is shown.
  $effect(() => {
    void name;
    const ed = editor;
    const marks = problems;
    untrack(() => ed?.mark(marks));
  });
</script>

<div class="rc-editor" bind:this={host} aria-busy={loading} hidden={loading && !ready}></div>

<style>
  .rc-editor {
    flex: 1;
    min-height: 120px;
    overflow: hidden;
    border: 1px solid var(--border);
    border-radius: 6px;
  }
  .rc-editor :global(.cm-editor) {
    height: 100%;
  }
  .rc-editor :global(.cm-scroller) {
    overflow: auto;
  }
</style>
