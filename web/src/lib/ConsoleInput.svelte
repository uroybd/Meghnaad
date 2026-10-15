<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { Completer } from './completer.svelte';
  import { COMMAND_NAMES } from './completion';
  import CompletionMenu from './CompletionMenu.svelte';
  import { ArrowRightToLine, Check, ChevronUp, Copy, CornerDownLeft, Pencil } from './icons';
  import { highlightPieces } from './highlight';
  import ReviewBar from './ReviewBar.svelte';
  import { review } from './review.svelte';
  import { store } from './store.svelte';
  import { expandTaskCommands } from './taskCommands';
  import { parseReview } from './tasksh';

  /** Focus the prompt when it appears (not on a phone, where that would raise the keyboard). */
  let { autofocus = false }: { autofocus?: boolean } = $props();

  let cursor = $state(-1); // -1 = the line being typed
  // What was half-typed survives a trip to another page: this prompt is rebuilt when the Console
  // opens or closes, since the Console carries its own.
  let line = $state(store.promptLine);
  $effect(() => {
    store.promptLine = line;
  });
  // Something outside asked for the prompt (the bulk bar's Command): take its text, and the focus.
  let asked = store.promptRequest?.n ?? 0;
  $effect(() => {
    const r = store.promptRequest;
    if (!r || r.n === asked) return;
    asked = r.n;
    line = r.text;
    cursor = -1;
    completer.close();
    el?.focus();
    queueMicrotask(() => el?.setSelectionRange(line.length, line.length));
  });
  onMount(() => {
    if (autofocus && !window.matchMedia('(pointer: coarse)').matches) el?.focus();
  });
  let saved = '';
  let el: HTMLTextAreaElement;
  const completer = new Completer();

  // The line is coloured: a copy of it drawn under the field, which has the text itself (transparent) and the caret.
  const commands = $derived(
    new Set([...COMMAND_NAMES, ...store.vocab.reports.map((r) => r.name), ...store.taskCommands.map((c) => c.name)]),
  );
  const coloured = $derived(highlightPieces(line, commands));
  // The field is as tall as its text: a long command wraps, and all of it can be seen.
  $effect(() => {
    void line;
    if (!el) return;
    el.style.height = 'auto';
    el.style.height = `${el.scrollHeight}px`;
  });

  // Edit the last command (typed or UI-driven) instead of retyping it.
  function reuse() {
    line = store.lastCommand.replace(/^task\s+/, '');
    cursor = -1;
    el.focus();
    queueMicrotask(() => el.setSelectionRange(line.length, line.length));
  }

  let copied = $state(false);
  async function copyLast() {
    try {
      await navigator.clipboard.writeText(store.lastCommand);
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch {
      store.notify('Could not copy to the clipboard.', 'err');
    }
  }

  /**
   * Empty the prompt, and the copy of it that the next prompt starts from. The copy is kept in step by an effect, which
   * has not run yet when a command switches the page (a command from the Tasks page opens the Console, which builds a
   * new prompt), so that one would start with the text that was just sent.
   */
  function clearLine() {
    line = '';
    store.promptLine = '';
  }

  function submit() {
    // While a review asks, the answers are keys (see `reviewKey`); only what to change is typed.
    if (review.captures) {
      if (review.phase === 'modifying') {
        const words = line;
        clearLine();
        cursor = -1;
        completer.close();
        void review.respond(words);
      }
      return;
    }
    const l = line.trim();
    if (!l) return;
    clearLine(); // the command moves to the "last command" line above the prompt
    cursor = -1;
    completer.close();
    if (l === 'clear' || l === 'cls') {
      store.clear();
      return;
    }
    // tasksh's `review` is the browser's own: it is not sent, it drives a session of ordinary commands.
    const asked = parseReview(l);
    if (asked) {
      store.view = 'console';
      void review.start({ limit: asked.limit, surface: 'console' });
      return;
    }
    // A task command from settings.json stands for the command line it names (what runs, and what the checks
    // below look at, is that); the history keeps what was typed.
    const cmd = expandTaskCommands(l, store.taskCommands);
    // Reports focus the Tasks view; everything else lands in the console scrollback.
    // Show the console first so a write or an error is visible; `run` switches back for reports.
    if (store.view !== 'console' && (consoleOnly(cmd) || !looksLikeReport(cmd))) store.view = 'console';
    store.run({ line: cmd }, l);
  }

  /** Commands with no page of their own: they only ever answer in the Console. */
  const CONSOLE_ONLY = new Set(['show', 'config', 'export', 'import', 'timesheet', 'information', 'colors', 'color']);

  function consoleOnly(l: string): boolean {
    const words = l
      .replace(/^task\s+/, '')
      .split(/\s+/)
      .filter((w) => !w.startsWith('rc.'));
    return words.some((w) => CONSOLE_ONLY.has(w) || /^g?history\./.test(w));
  }

  /** Cheap client-side guess, only to avoid flashing the Console tab for a report. */
  function looksLikeReport(l: string): boolean {
    const words = l.replace(/^task\s+/, '').split(/\s+/);
    return store.reports.some((r) => words.includes(r.name));
  }

  function edit(e: { line: string; caret: number } | null) {
    if (!e) return false;
    line = e.line;
    queueMicrotask(() => el.setSelectionRange(e.caret, e.caret));
    return true;
  }

  function older() {
    const h = store.history;
    if (h.length === 0) return false;
    if (cursor === -1) saved = line;
    cursor = Math.min(h.length - 1, cursor + 1);
    line = h[h.length - 1 - cursor];
    queueMicrotask(() => el.setSelectionRange(line.length, line.length));
    return true;
  }

  // Tab and ↑ have buttons too, for touch screens that have neither key.
  function tapComplete() {
    edit(completer.tab(line, el.selectionStart ?? line.length, store.vocab));
    el.focus();
  }
  function tapOlder() {
    older();
    el.focus();
  }

  /**
   * While a review asks, a key is the answer: tasksh's menu is keybindings, not a shell, so nothing is typed and no
   * Enter follows. (What to change is typed, and Escape cancels that.) Keys of buttons and of other fields are theirs.
   */
  function reviewKey(e: KeyboardEvent): boolean {
    if (!review.captures || e.defaultPrevented || e.metaKey || e.ctrlKey || e.altKey) return false;
    // The target is not always an element (a key can come to the document or the window).
    const t = e.target instanceof Element ? e.target : null;
    if (t?.closest('button')) return false;
    if (t && t !== el && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.tagName === 'SELECT')) return false;
    if (review.phase === 'modifying') {
      if (e.key !== 'Escape') return false;
      e.preventDefault();
      void review.modify('');
      return true;
    }
    if (e.key === 'Escape') {
      e.preventDefault();
      review.quit();
      return true;
    }
    if (e.key !== 'Enter' && e.key.length !== 1) return false; // arrows, Tab, function keys: left alone
    e.preventDefault();
    void review.press(e.key);
    return true;
  }

  // The prompt is off while a review asks, except to type the change: it has the focus then.
  const keysOnly = $derived(review.captures && review.phase !== 'modifying');
  $effect(() => {
    if (review.captures && review.phase === 'modifying') void tick().then(() => el?.focus());
  });

  function onkeydown(e: KeyboardEvent) {
    if (reviewKey(e)) return;
    const open = !!completer.menu;
    if (e.key === 'Enter') {
      e.preventDefault();
      if (open && completer.menu!.sel >= 0)
        completer.close(); // accept the highlighted option
      else submit();
    } else if (e.key === 'Tab') {
      if (edit(completer.tab(line, el.selectionStart ?? line.length, store.vocab, e.shiftKey))) e.preventDefault();
    } else if ((e.key === 'ArrowDown' || e.key === 'ArrowUp') && open) {
      e.preventDefault();
      edit(completer.move(e.key === 'ArrowDown' ? 1 : -1));
    } else if (e.key === 'ArrowUp') {
      if (older()) e.preventDefault();
    } else if (e.key === 'ArrowDown') {
      if (cursor === -1) return;
      e.preventDefault();
      cursor -= 1;
      line = cursor === -1 ? saved : store.history[store.history.length - 1 - cursor];
      queueMicrotask(() => el.setSelectionRange(line.length, line.length));
    } else if (e.key === 'l' && e.ctrlKey) {
      e.preventDefault();
      store.clear();
    } else if (e.key === 'Escape') {
      if (open) completer.close();
      else el.blur();
    }
  }

  // `/` focuses the prompt from anywhere that isn't a text field.
  function globalKey(e: KeyboardEvent) {
    if (reviewKey(e)) return;
    const t = e.target as HTMLElement | null;
    const typing =
      t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.tagName === 'SELECT' || t.isContentEditable);
    if (e.key === '/' && !typing && !e.metaKey && !e.ctrlKey) {
      e.preventDefault();
      el.focus();
      el.select();
    }
  }
</script>

<svelte:window onkeydown={globalKey} />

<form
  class="prompt"
  onsubmit={(e) => {
    e.preventDefault();
    submit();
  }}
>
  <!-- The last command, and the buttons for the keys a phone has not (↑ and Tab), which are only shown on one. -->
  <div
    class="last row"
    class:empty={!store.lastCommand}
    title="The last command run, from the console or from the UI. ↑ recalls it."
  >
    {#if store.lastCommand}
      <span class="dim">last</span>
      <code class="mono grow" data-testid="last-command">{store.lastCommand}</code>
    {:else}
      <span class="grow"></span>
    {/if}
    <button type="button" class="ghost touch" aria-label="Previous command" title="Previous command" onclick={tapOlder}
      ><ChevronUp size={16} /></button
    >
    <button type="button" class="ghost touch" aria-label="Complete" title="Complete (Tab)" onclick={tapComplete}
      ><ArrowRightToLine size={16} /></button
    >
    {#if store.lastCommand}
      <button type="button" class="ghost" onclick={reuse} title="Edit this command" aria-label="Edit last command"
        ><Pencil size={13} /></button
      >
      <button type="button" class="ghost" onclick={copyLast} title="Copy" aria-label="Copy last command">
        {#if copied}<Check size={13} />{:else}<Copy size={13} />{/if}
      </button>
    {/if}
  </div>
  {#if review.captures}<ReviewBar />{/if}
  <CompletionMenu {completer} placement="above" onpick={(i) => edit(completer.pick(i))} />
  <label class="row">
    <span class="mono ps" aria-hidden="true">{review.captures ? 'review>' : 'task>'}</span>
    <span class="sr-only">Command</span>
    <span class="field grow">
      <span class="mirror mono" aria-hidden="true"
        >{#each coloured as piece, i (i)}<span class="hl-{piece.kind}">{piece.text}</span>{/each}</span
      >
      <textarea
        bind:this={el}
        bind:value={line}
        {onkeydown}
        onblur={() => completer.close()}
        oninput={() => {
          // A line of commands has no line breaks: a pasted one becomes spaces.
          if (line.includes('\n')) line = line.replace(/\r?\n/g, ' ');
          cursor = -1;
          if (review.captures) completer.close();
          else completer.suggest(line, el.selectionStart ?? line.length, store.vocab);
        }}
        rows="1"
        class="mono"
        disabled={keysOnly}
        placeholder={keysOnly
          ? 'Reviewing: press a key, or use the buttons above'
          : review.captures
            ? '+tag -tag /teh/the/ project:X'
            : 'next · add Buy milk due:tomorrow · project:Home +work list   ( / to focus )'}
        autocomplete="off"
        autocapitalize="off"
        spellcheck="false"
        enterkeyhint="go"
        data-testid="prompt"></textarea>
    </span>
    <button class="primary btn" aria-label="Run"><CornerDownLeft size={15} /> <span class="run">Run</span></button>
  </label>
</form>

<style>
  .prompt {
    position: relative;
    border-top: 1px solid var(--line);
    background: var(--panel);
    padding: 12px 24px 14px;
  }
  .prompt :global(.menu) {
    left: 24px;
  }
  .last {
    font-size: 13px;
    margin-bottom: 8px;
    padding: 4px 12px;
    background: var(--panel-2);
    border-radius: 6px;
  }
  /* Nothing to show on a computer: the buttons of a phone are all this row would have. */
  .last.empty {
    display: none;
  }
  .last code {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .last button {
    padding: 2px 6px;
    line-height: 0;
    color: var(--dim);
  }
  .btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  .touch {
    display: none;
    color: var(--dim);
  }
  .ps {
    color: var(--accent);
    font-weight: 700;
  }
  /* The field is the textarea (the text, transparent, and the caret) over a copy of the line in colours: the two have the same
     font, padding and wrapping, so every letter is where the other has it. */
  .field {
    position: relative;
    display: block;
    min-width: 0;
  }
  .mirror,
  textarea {
    box-sizing: border-box;
    font-size: 16px;
    line-height: 1.4;
    padding: 6px 10px;
    border: 1px solid transparent;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    letter-spacing: normal;
  }
  .mirror {
    position: absolute;
    inset: 0;
    overflow: hidden;
    pointer-events: none;
    color: var(--text);
  }
  textarea {
    position: relative;
    display: block;
    width: 100%;
    resize: none;
    overflow: hidden;
    background: transparent;
    color: transparent;
    caret-color: var(--text);
  }
  textarea::placeholder {
    color: var(--dim);
  }
  .mirror :global(.hl-command) {
    color: var(--accent);
    font-weight: 600;
  }
  .mirror :global(.hl-keyword),
  .mirror :global(.hl-punct),
  .mirror :global(.hl-config) {
    color: var(--dim);
  }
  .mirror :global(.hl-config) {
    font-style: italic;
  }
  .mirror :global(.hl-attr) {
    color: var(--hl-attr);
  }
  .mirror :global(.hl-tag) {
    color: var(--ok);
  }
  .mirror :global(.hl-id) {
    color: var(--warn);
  }
  .mirror :global(.hl-op) {
    color: var(--err);
    font-weight: 600;
  }
  .mirror :global(.hl-str) {
    color: var(--hl-str);
  }
  @media (max-width: 760px) {
    .prompt {
      padding: 6px max(8px, env(safe-area-inset-right)) max(6px, env(safe-area-inset-bottom))
        max(8px, env(safe-area-inset-left));
    }
    .prompt :global(.menu) {
      left: 8px;
      right: 8px;
      max-width: none;
    }
    .run {
      display: none;
    }
    .touch {
      display: inline-flex;
      align-items: center;
      justify-content: center;
    }
    .last {
      margin-bottom: 2px;
    }
    .last.empty {
      display: flex;
      margin-bottom: 2px;
    }
    .ps {
      display: none;
    }
  }
</style>
