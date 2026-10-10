<script lang="ts">
  import { Completer } from './completer.svelte';
  import CompletionMenu from './CompletionMenu.svelte';
  import { choose, isNew, suggest, wordAt } from './suggest';

  /**
   * A text field that offers names as you type (a project, or a list of tags) and takes anything else too: the
   * offers are a help, and a name nobody has used yet is how a new one is made. ↑/↓ pick an offer, Enter or Tab
   * takes it, Esc puts the offers away (and nothing else), and a click or tap takes one as well.
   */
  let {
    value = $bindable(''),
    names,
    id,
    noun,
    list = false,
    placeholder = '',
  }: {
    value: string;
    /** What to offer. */
    names: string[];
    id: string;
    /** What the field holds, for "a new project". */
    noun: string;
    /** Several names, separated by spaces or commas, each offered on its own. */
    list?: boolean;
    placeholder?: string;
  } = $props();

  const completer = new Completer();
  const menuId = $derived(`${id}-menu`);
  let el: HTMLInputElement;
  // The word being typed, to say when it is a name nobody has used.
  let typed = $state('');

  function show() {
    const w = wordAt(value, el?.selectionStart ?? value.length, list);
    typed = w.text;
    // In a list, what is already there is not offered again.
    const rest = value.slice(0, w.start) + ' ' + value.slice(w.end);
    const taken = new Set(list ? rest.split(/[\s,]+/).map((t) => t.replace(/^\+/, '')) : []);
    const options = suggest(names, w.text, taken);
    completer.menu = options.length ? { base: value, start: w.start, end: w.end, options, sel: -1 } : null;
  }

  function accept(i: number) {
    const m = completer.menu;
    if (!m || !m.options[i]) return;
    const r = choose(value, { start: m.start, end: m.end, text: '' }, m.options[i].value, list);
    value = r.value;
    completer.close();
    typed = '';
    queueMicrotask(() => el.setSelectionRange(r.caret, r.caret));
  }

  function onkeydown(e: KeyboardEvent) {
    const m = completer.menu;
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      if (!m) show();
      const open = completer.menu;
      if (!open) return;
      e.preventDefault();
      const n = open.options.length;
      const down = e.key === 'ArrowDown';
      open.sel = open.sel < 0 ? (down ? 0 : n - 1) : (open.sel + (down ? 1 : -1) + n) % n;
    } else if ((e.key === 'Enter' || e.key === 'Tab') && m && m.sel >= 0) {
      // Only an offer you moved to is taken; otherwise Enter submits the form and Tab moves on, with what you typed.
      e.preventDefault();
      accept(m.sel);
    } else if (e.key === 'Escape' && m) {
      e.preventDefault();
      e.stopPropagation();
      completer.close();
    }
  }
</script>

<div class="suggest">
  <input
    {id}
    bind:this={el}
    bind:value
    {placeholder}
    {onkeydown}
    oninput={show}
    onfocus={show}
    onclick={show}
    onblur={() => {
      completer.close();
      typed = '';
    }}
    role="combobox"
    aria-autocomplete="list"
    aria-expanded={!!completer.menu}
    aria-controls={menuId}
    autocomplete="off"
    autocapitalize="off"
    spellcheck="false"
  />
  <CompletionMenu {completer} id={menuId} label="Suggested {noun} names" placement="below" onpick={accept} />
  {#if isNew(names, typed)}<div class="dim hint">“{typed}” will be a new {noun}.</div>{/if}
</div>

<style>
  .suggest {
    position: relative;
  }
  .suggest input {
    width: 100%;
    box-sizing: border-box;
  }
  .hint {
    font-size: 12px;
    margin-top: 2px;
  }
</style>
