<script lang="ts">
  import { Completer } from './completer.svelte';
  import CompletionMenu from './CompletionMenu.svelte';
  import { X } from './icons';
  import { isNew, suggest } from './suggest';

  /**
   * A field for a list of tags: each one finished (by a space, a comma, Enter, Tab, choosing an offer or leaving the
   * field) is a pill with a × to take it off, and the word being typed is offered names as in `SuggestInput`.
   * Backspace in an empty word takes the last pill back to edit. `value` is the tags separated by spaces, the word
   * being typed included, so a form that reads it never misses one.
   */
  let {
    value = $bindable(''),
    names,
    id,
    noun = 'tag',
    placeholder = '',
  }: {
    value: string;
    /** What to offer. */
    names: string[];
    id: string;
    noun?: string;
    placeholder?: string;
  } = $props();

  const SEPARATOR = /[\s,]+/;
  const clean = (t: string) => t.trim().replace(/^\+/, '');
  const split = (v: string) => [...new Set(v.split(SEPARATOR).map(clean).filter(Boolean))];

  let tags = $state<string[]>(split(value));
  let draft = $state('');
  const composed = () => [...tags, clean(draft)].filter(Boolean).join(' ');

  // A value set from outside (a reset) is read again; the field's own changes are the last value it saw.
  let seen = value;
  $effect(() => {
    if (value !== seen) {
      seen = value;
      tags = split(value);
      draft = '';
    }
  });

  const completer = new Completer();
  const menuId = $derived(`${id}-menu`);
  let el: HTMLInputElement;

  function sync() {
    value = seen = composed();
  }

  function add(words: string[]) {
    for (const t of words.map(clean).filter(Boolean)) if (!tags.includes(t)) tags.push(t);
  }

  function remove(i: number) {
    tags.splice(i, 1);
    sync();
    el?.focus();
  }

  /** Make the word being typed a pill. */
  function commit() {
    add([draft]);
    draft = '';
    sync();
  }

  function show() {
    const options = suggest(names, clean(draft), new Set(tags));
    completer.menu = options.length ? { base: draft, start: 0, end: draft.length, options, sel: -1 } : null;
  }

  function oninput() {
    // A separator typed or pasted finishes every word before it.
    const parts = draft.split(SEPARATOR);
    if (parts.length > 1) {
      add(parts.slice(0, -1));
      draft = parts[parts.length - 1];
    }
    sync();
    show();
  }

  function accept(i: number) {
    const m = completer.menu;
    if (!m || !m.options[i]) return;
    add([m.options[i].value]);
    draft = '';
    sync();
    completer.close();
    el?.focus();
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
      e.preventDefault();
      accept(m.sel);
    } else if (e.key === 'Enter' || e.key === 'Tab') {
      // Otherwise Enter submits the form and Tab moves on, with the word typed as a tag.
      commit();
      completer.close();
    } else if (e.key === 'Escape' && m) {
      e.preventDefault();
      e.stopPropagation();
      completer.close();
    } else if (e.key === 'Backspace' && draft === '' && tags.length && el.selectionStart === 0) {
      e.preventDefault();
      draft = tags.pop() ?? '';
      sync();
      show();
    }
  }
</script>

<div class="suggest">
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -- a click anywhere in the box focuses its input; the keyboard reaches the input directly -->
  <div class="box" onclick={() => el?.focus()}>
    {#each tags as t, i (t)}
      <span class="tagpill"
        >{t}<button type="button" class="drop" aria-label="Remove tag {t}" title="Remove {t}" onclick={() => remove(i)}
          ><X size={12} /></button
        ></span
      >
    {/each}
    <input
      {id}
      bind:this={el}
      bind:value={draft}
      placeholder={tags.length ? '' : placeholder}
      {onkeydown}
      {oninput}
      onfocus={show}
      onclick={show}
      onblur={() => {
        completer.close();
        commit();
      }}
      role="combobox"
      aria-autocomplete="list"
      aria-expanded={!!completer.menu}
      aria-controls={menuId}
      autocomplete="off"
      autocapitalize="off"
      spellcheck="false"
    />
  </div>
  <CompletionMenu {completer} id={menuId} label="Suggested {noun} names" placement="below" onpick={accept} />
  {#if isNew(names, clean(draft))}<div class="dim hint">“{clean(draft)}” will be a new {noun}.</div>{/if}
</div>

<style>
  .suggest {
    position: relative;
  }
  .box {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
    box-sizing: border-box;
    width: 100%;
    min-height: 34px;
    padding: 3px 6px;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 6px;
    cursor: text;
  }
  .box:focus-within {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .box .tagpill {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    margin: 0;
    padding-right: 2px;
  }
  .drop {
    display: inline-flex;
    padding: 1px;
    border: 0;
    border-radius: 999px;
    background: transparent;
    color: inherit;
    opacity: 0.7;
  }
  .drop:hover:not(:disabled) {
    opacity: 1;
    background: color-mix(in srgb, var(--accent) 20%, transparent);
  }
  .box input {
    flex: 1 1 80px;
    min-width: 80px;
    border: 0;
    padding: 3px 4px;
    background: transparent;
  }
  .box input:focus {
    outline: none;
  }
  .hint {
    font-size: 12px;
    margin-top: 2px;
  }
</style>
