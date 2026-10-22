<script lang="ts">
  import { tick } from 'svelte';
  import CompletionMenu from './CompletionMenu.svelte';
  import { dateWords, type Option } from './completion';
  import { fromParts } from './dates';
  import { Calendar, X } from './icons';

  /**
   * A date, shown as a button that opens a small panel to choose it in. The panel has a field to type it (a date as Taskwarrior
   * reads one: `sow`, `eod`, `eow-2d`, `2026-10-01`; Tab or a tap completes the keywords, which are the engine's) and, under
   * it, a calendar day and an optional time. What is typed is kept as typed; the calendar writes `YYYY-MM-DD`, or
   * `YYYY-MM-DDTHH:MM` with a time. Empty is no date.
   */
  let {
    value = $bindable(''),
    label,
    id = undefined,
    placeholder = 'Set date',
    invalid = false,
  }: { value?: string; label: string; id?: string; placeholder?: string; invalid?: boolean } = $props();

  const KEYWORDS = ['now', 'today', 'tomorrow', 'eod', 'sow', 'eow', 'som', 'eom'];

  let open = $state(false);
  let button: HTMLButtonElement | undefined = $state();
  let panel: HTMLElement | undefined = $state();
  let field: HTMLInputElement | undefined = $state();
  let place = $state<{ left: number; top?: number; bottom?: number }>({ left: 8, top: 0 });
  /** The date words the typed text could become (the engine's), and the one Tab or an arrow has chosen. */
  let options = $state<Option[]>([]);
  let sel = $state(-1);
  const menu = $derived(options.length ? { options, sel } : null);
  const suggest = () => {
    options = dateWords(value);
    sel = -1;
  };
  /** Choose the next (or previous) option, writing it in the field. */
  function step(delta: 1 | -1) {
    const n = options.length;
    sel = sel < 0 ? (delta === 1 ? 0 : n - 1) : (sel + delta + n) % n;
    value = options[sel].value;
  }

  const date = $derived(/^\d{4}-\d{2}-\d{2}/.test(value) ? value.slice(0, 10) : '');
  const time = $derived(date && /^.{10}[T ]\d{2}:\d{2}/.test(value) ? value.slice(11, 16) : '');
  /** Typed as an expression (a keyword, arithmetic): the day and time below stay empty rather than guess. */
  const expression = $derived(!!value.trim() && !date);

  async function show() {
    if (!button) return;
    const r = button.getBoundingClientRect();
    const up = window.innerHeight - r.bottom < 340 && r.top > window.innerHeight - r.bottom;
    place = {
      left: Math.max(8, Math.min(r.left, window.innerWidth - 320 - 8)),
      ...(up ? { bottom: window.innerHeight - r.top + 4 } : { top: r.bottom + 4 }),
    };
    open = true;
    await tick();
    field?.focus();
  }
  function close(refocus = false) {
    open = false;
    options = [];
    if (refocus) button?.focus();
  }

  function onkeydown(e: KeyboardEvent) {
    if (!open) return;
    if (e.target !== field) {
      if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        close(true);
      }
      return;
    }
    if (e.key === 'Tab' && !e.altKey && options.length) {
      e.preventDefault();
      step(e.shiftKey ? -1 : 1);
    } else if ((e.key === 'ArrowDown' || e.key === 'ArrowUp') && options.length) {
      e.preventDefault();
      step(e.key === 'ArrowDown' ? 1 : -1);
    } else if (e.key === 'Enter' || e.key === 'Escape') {
      e.preventDefault();
      if (e.key === 'Escape') e.stopPropagation();
      if (options.length) options = [];
      else close(true);
    }
  }
  function away(e: Event) {
    if (open && !panel?.contains(e.target as Node) && !button?.contains(e.target as Node)) close();
  }
</script>

<svelte:window onpointerdown={away} onresize={() => open && close()} {onkeydown} />

<span class="datefield">
  <button
    bind:this={button}
    {id}
    type="button"
    class="trigger"
    class:empty={!value}
    class:mono={!!value}
    aria-haspopup="dialog"
    aria-expanded={open}
    aria-label={value ? `${label}: ${value}` : label}
    class:invalid
    title={label}
    onclick={() => (open ? close() : void show())}
  >
    <span class="text">{value || placeholder}</span>
    <Calendar size={14} />
  </button>
  {#if open}
    <div
      bind:this={panel}
      class="panel"
      role="dialog"
      aria-label={label}
      style:left="{place.left}px"
      style:top={place.top === undefined ? undefined : `${place.top}px`}
      style:bottom={place.bottom === undefined ? undefined : `${place.bottom}px`}
    >
      <div class="typed">
        <label class="sr-only" for="datefield-input">{label}</label>
        <input
          id="datefield-input"
          class="mono"
          bind:this={field}
          bind:value
          oninput={suggest}
          onblur={() => (options = [])}
          placeholder="sow, eod, 2026-10-01 …  (Tab completes)"
          autocomplete="off"
          spellcheck="false"
          aria-invalid={invalid}
        />
        <CompletionMenu
          completer={{ menu }}
          placement="below"
          label="Date words"
          onpick={(i) => {
            value = options[i].value;
            options = [];
          }}
        />
      </div>
      <div class="words" role="group" aria-label="Keywords">
        {#each KEYWORDS as w (w)}
          <button type="button" class="ghost word mono" class:on={value.trim() === w} onclick={() => (value = w)}
            >{w}</button
          >
        {/each}
      </div>
      <div class="pick" role="group" aria-label="From the calendar">
        <input
          type="date"
          value={date}
          aria-label="{label}: day"
          oninput={(e) => (value = fromParts({ date: e.currentTarget.value, time }))}
        />
        <input
          type="time"
          value={time}
          disabled={!date}
          aria-label="{label}: time (optional)"
          title="Optional. Leave empty for the whole day."
          oninput={(e) => (value = fromParts({ date, time: e.currentTarget.value }))}
        />
      </div>
      {#if expression}<p class="dim hint">Typed as a date expression; pick a day to replace it.</p>{/if}
      <div class="foot">
        <button type="button" class="ghost" disabled={!value} onclick={() => (value = '')}><X size={13} /> Clear</button
        >
        <button type="button" onclick={() => close(true)}>Done</button>
      </div>
    </div>
  {/if}
</span>

<style>
  .datefield {
    display: inline-flex;
  }
  .trigger {
    display: inline-flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    min-width: 9.5em;
    padding: 5px 9px;
    font-size: 13px;
    text-align: left;
  }
  .trigger.empty .text {
    color: var(--dim);
  }
  .trigger.invalid {
    border-color: var(--err);
  }
  .panel {
    position: fixed;
    z-index: 40;
    width: min(320px, calc(100vw - 16px));
    box-sizing: border-box;
    display: grid;
    gap: 8px;
    padding: 10px;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    box-shadow: 0 6px 22px rgb(0 0 0 / 0.22);
  }
  .typed {
    position: relative;
  }
  .typed input {
    width: 100%;
    box-sizing: border-box;
  }
  .words {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .word {
    padding: 2px 8px;
    font-size: 12.5px;
    border: 1px solid var(--line);
    border-radius: 999px;
  }
  .word.on {
    border-color: var(--accent);
    color: var(--accent);
  }
  .pick {
    display: flex;
    gap: 6px;
  }
  .pick input[type='date'] {
    flex: 1 1 auto;
    min-width: 0;
  }
  .pick input[type='time'] {
    flex: 0 0 7em;
  }
  .hint {
    margin: 0;
    font-size: 12px;
  }
  .foot {
    display: flex;
    justify-content: space-between;
    gap: 8px;
  }
  .foot button {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
</style>
