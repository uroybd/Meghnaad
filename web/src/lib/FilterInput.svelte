<script lang="ts">
  import { Completer } from './completer.svelte';
  import CompletionMenu from './CompletionMenu.svelte';
  import { store } from './store.svelte';

  /**
   * The filter box shared by the Tasks, Summary, Calendar and Burndown pages: Taskwarrior's filter
   * syntax, with Tab completion of projects, tags and attributes.
   */
  let {
    value = $bindable(''),
    id,
    placeholder = 'Add filters: project:Home +work due.before:eow  (Tab completes)',
    autofocus = false,
  }: { value: string; id: string; placeholder?: string; autofocus?: boolean } = $props();

  const completer = new Completer();
  let el: HTMLInputElement;

  // Focus the box when the page opens, so you can start typing. Not on a phone: that would raise the
  // on-screen keyboard over the page you just opened.
  $effect(() => {
    if (autofocus && !window.matchMedia('(pointer: coarse)').matches) el?.focus();
  });

  function onkeydown(e: KeyboardEvent) {
    const open = !!completer.menu;
    const apply = (r: { line: string; caret: number } | null) => {
      if (!r) return false;
      value = r.line;
      queueMicrotask(() => el.setSelectionRange(r.caret, r.caret));
      return true;
    };
    if (e.key === 'Tab') {
      if (apply(completer.tab(value, el.selectionStart ?? value.length, store.vocab, e.shiftKey))) e.preventDefault();
    } else if ((e.key === 'ArrowDown' || e.key === 'ArrowUp') && open) {
      e.preventDefault();
      apply(completer.move(e.key === 'ArrowDown' ? 1 : -1));
    } else if (e.key === 'Enter' && open) {
      e.preventDefault();
      completer.close();
    } else if (e.key === 'Escape') {
      completer.close();
    }
  }
</script>

<div class="filter">
  <label class="sr-only" for={id}>Filter</label>
  <input
    {id}
    bind:this={el}
    bind:value
    {onkeydown}
    oninput={() => completer.close()}
    onblur={() => completer.close()}
    {placeholder}
    autocomplete="off"
    spellcheck="false"
    class="mono"
  />
  <CompletionMenu
    {completer}
    placement="below"
    onpick={(i) => {
      const r = completer.pick(i);
      if (r) value = r.line;
    }}
  />
</div>

<style>
  .filter {
    position: relative;
    flex: 1 1 auto;
    min-width: 0;
  }
  .filter input {
    width: 100%;
  }
</style>
