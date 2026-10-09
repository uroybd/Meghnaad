<script lang="ts">
  import { onMount } from 'svelte';
  import { Completer } from './completer.svelte';
  import CompletionMenu from './CompletionMenu.svelte';
  import { ArrowRightToLine, Check, ChevronUp, Copy, CornerDownLeft, Pencil } from './icons';
  import { store } from './store.svelte';

  /** Focus the prompt when it appears (not on a phone, where that would raise the keyboard). */
  let { autofocus = false }: { autofocus?: boolean } = $props();

  let cursor = $state(-1); // -1 = the line being typed
  // What was half-typed survives a trip to another page: this prompt is rebuilt when the Console
  // opens or closes, since the Console carries its own.
  let line = $state(store.promptLine);
  $effect(() => {
    store.promptLine = line;
  });
  onMount(() => {
    if (autofocus && !window.matchMedia('(pointer: coarse)').matches) el?.focus();
  });
  let saved = '';
  let el: HTMLInputElement;
  const completer = new Completer();

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

  function submit() {
    const l = line.trim();
    if (!l) return;
    line = ''; // the command moves to the "last command" line above the prompt
    cursor = -1;
    completer.close();
    if (l === 'clear' || l === 'cls') {
      store.clear();
      return;
    }
    // Reports focus the Tasks view; everything else lands in the console scrollback.
    // Show the console first so a write or an error is visible; `run` switches back for reports.
    if (store.view !== 'console' && (consoleOnly(l) || !looksLikeReport(l))) store.view = 'console';
    store.run({ line: l });
  }

  /** `show` and `config` have no page of their own; they only ever answer in the Console. */
  function consoleOnly(l: string): boolean {
    const words = l.replace(/^task\s+/, '').split(/\s+/).filter((w) => !w.startsWith('rc.'));
    return words.some((w) => w === 'show' || w === 'config');
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

  function onkeydown(e: KeyboardEvent) {
    const open = !!completer.menu;
    if (e.key === 'Enter') {
      e.preventDefault();
      if (open && completer.menu!.sel >= 0) completer.close(); // accept the highlighted option
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
    const t = e.target as HTMLElement | null;
    const typing = t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.tagName === 'SELECT' || t.isContentEditable);
    if (e.key === '/' && !typing && !e.metaKey && !e.ctrlKey) {
      e.preventDefault();
      el.focus();
      el.select();
    }
  }
</script>

<svelte:window onkeydown={globalKey} />

<form class="prompt" onsubmit={(e) => { e.preventDefault(); submit(); }}>
  {#if store.lastCommand}
    <div class="last row" title="The last command run, from the console or from the UI. ↑ recalls it.">
      <span class="dim">last</span>
      <code class="mono grow" data-testid="last-command">{store.lastCommand}</code>
      <button type="button" class="ghost" onclick={reuse} title="Edit this command" aria-label="Edit last command"><Pencil size={13} /></button>
      <button type="button" class="ghost" onclick={copyLast} title="Copy" aria-label="Copy last command">
        {#if copied}<Check size={13} />{:else}<Copy size={13} />{/if}
      </button>
    </div>
  {/if}
  <CompletionMenu {completer} placement="above" onpick={(i) => edit(completer.pick(i))} />
  <label class="row">
    <span class="mono ps" aria-hidden="true">task&gt;</span>
    <span class="sr-only">Command</span>
    <input
      bind:this={el}
      bind:value={line}
      {onkeydown}
      onblur={() => completer.close()}
      oninput={() => { completer.close(); cursor = -1; }}
      class="grow mono"
      placeholder="next · add Buy milk due:tomorrow · project:Home +work list   ( / to focus )"
      autocomplete="off"
      autocapitalize="off"
      spellcheck="false"
      enterkeyhint="go"
      data-testid="prompt"
    />
    <button type="button" class="ghost touch" aria-label="Previous command" title="Previous command" onclick={tapOlder}><ChevronUp size={18} /></button>
    <button type="button" class="ghost touch" aria-label="Complete" title="Complete (Tab)" onclick={tapComplete}><ArrowRightToLine size={18} /></button>
    <button class="primary btn" aria-label="Run"><CornerDownLeft size={15} /> <span class="run">Run</span></button>
  </label>
</form>

<style>
  .prompt { position: relative; border-top: 1px solid var(--line); background: var(--panel); padding: 12px 24px 14px; }
  .prompt :global(.menu) { left: 24px; }
  .last { font-size: 13px; margin-bottom: 8px; padding: 4px 12px; background: var(--panel-2); border-radius: 6px; }
  .last code { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .last button { padding: 2px 6px; line-height: 0; color: var(--dim); }
  .btn { display: inline-flex; align-items: center; gap: 5px; }
  .touch { display: none; color: var(--dim); }
  .ps { color: var(--accent); font-weight: 700; }
  input { border-color: transparent; background: transparent; font-size: 16px; }
  @media (max-width: 760px) {
    .prompt {
      padding: 6px max(8px, env(safe-area-inset-right)) max(6px, env(safe-area-inset-bottom)) max(8px, env(safe-area-inset-left));
    }
    .prompt :global(.menu) { left: 8px; right: 8px; max-width: none; }
    .run { display: none; }
    .touch { display: inline-flex; align-items: center; justify-content: center; }
    .last { margin-bottom: 2px; }
    input { font-size: 16px; }
    .ps { display: none; }
  }
</style>
