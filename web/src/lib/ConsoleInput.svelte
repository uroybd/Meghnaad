<script lang="ts">
  import { Completer } from './completer.svelte';
  import CompletionMenu from './CompletionMenu.svelte';
  import { Check, Copy, CornerDownLeft, Pencil } from './icons';
  import { store } from './store.svelte';

  let cursor = $state(-1); // -1 = the line being typed
  let line = $state('');
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
    if (store.view === 'tasks' && !looksLikeReport(l)) store.view = 'console';
    store.run({ line: l });
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
      const h = store.history;
      if (h.length === 0) return;
      e.preventDefault();
      if (cursor === -1) saved = line;
      cursor = Math.min(h.length - 1, cursor + 1);
      line = h[h.length - 1 - cursor];
      queueMicrotask(() => el.setSelectionRange(line.length, line.length));
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
  <CompletionMenu {completer} placement="above" />
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
    <button class="primary btn" aria-label="Run"><CornerDownLeft size={15} /> Run</button>
  </label>
</form>

<style>
  .prompt { position: relative; border-top: 1px solid var(--line); background: var(--panel); padding: 8px 12px; }
  .prompt :global(.menu) { left: 12px; }
  .last { font-size: 12px; margin-bottom: 4px; padding: 2px 8px; background: var(--panel-2); border-radius: 6px; }
  .last code { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .last button { padding: 2px 6px; line-height: 0; color: var(--dim); }
  .btn { display: inline-flex; align-items: center; gap: 5px; }
  .ps { color: var(--accent); font-weight: 700; }
  input { border-color: transparent; background: transparent; font-size: 15px; }
</style>
