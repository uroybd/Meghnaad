<script lang="ts">
  import { tick } from 'svelte';
  import { Repeat, X } from './icons';
  import { PRESETS } from './recurrence';
  import {
    buildRule,
    DAY_NAMES,
    defaultRule,
    describeIter,
    monthChoices,
    nthWords,
    parseRule,
    WEEKDAYS,
    type CalendarRule,
    type Freq,
  } from './iterRule';

  /**
   * An iterative task's rule (`iter`), as a button that opens a panel to choose it in, as a date field does. Two modes:
   * Taskwarrior (its periods as pills, or anything `task` takes, typed: `3d`, `P2W`, "every other day"), and Calendar,
   * which builds an RRULE as calendar apps do: repeat every N days, weeks, months or years; on which weekdays or which
   * day of the month; ending never, on a day, or after a number of times. `anchor` is the day a new rule counts from
   * (the task's due date, else today).
   */
  let {
    value = $bindable(''),
    anchor,
    id = undefined,
    label = 'Iter',
  }: { value?: string; anchor: string; id?: string; label?: string } = $props();

  type Mode = 'taskwarrior' | 'calendar';
  let open = $state(false);
  let mode = $state<Mode>('taskwarrior');
  let rule = $state<CalendarRule | null>(null);
  /** What the Taskwarrior mode had, kept while the Calendar mode is tried, so switching back restores it. */
  let typed = $state('');
  let button: HTMLButtonElement | undefined = $state();
  let panel: HTMLElement | undefined = $state();
  let place = $state<{ left: number; top?: number; bottom?: number }>({ left: 8, top: 0 });

  const words = $derived(value.trim() ? describeIter(value) : '');
  const months = $derived(monthChoices(anchor));

  async function show() {
    if (!button) return;
    const parsed = parseRule(value);
    mode = parsed ? 'calendar' : 'taskwarrior';
    rule = parsed;
    typed = parsed ? '' : value;
    const r = button.getBoundingClientRect();
    const up = window.innerHeight - r.bottom < 460 && r.top > window.innerHeight - r.bottom;
    place = {
      left: Math.max(8, Math.min(r.left, window.innerWidth - 340 - 8)),
      ...(up ? { bottom: window.innerHeight - r.top + 4 } : { top: r.bottom + 4 }),
    };
    open = true;
    await tick();
    panel?.querySelector<HTMLElement>('[data-first]')?.focus();
  }
  function close(refocus = false) {
    open = false;
    if (refocus) button?.focus();
  }
  function away(e: Event) {
    if (open && !panel?.contains(e.target as Node) && !button?.contains(e.target as Node)) close();
  }
  function onkeydown(e: KeyboardEvent) {
    if (open && e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      close(true);
    }
  }

  function switchTo(m: Mode) {
    if (m === mode) return;
    mode = m;
    if (m === 'calendar') {
      typed = value;
      rule = parseRule(value) ?? defaultRule(anchor);
      value = buildRule(rule);
    } else {
      value = typed;
    }
  }
  /** Change the Calendar mode's rule and write it. */
  function edit(change: Partial<CalendarRule>) {
    if (!rule) return;
    rule = { ...rule, ...change };
    value = buildRule(rule);
  }
  function setFreq(freq: Freq) {
    const m = months;
    edit({
      freq,
      days: freq === 'weekly' ? (rule?.days.length ? rule.days : defaultRule(anchor).days) : [],
      month_day: freq === 'monthly' ? (m?.day ?? null) : null,
      nth: null,
      weekday: null,
    });
  }
  function toggleDay(d: string) {
    if (!rule) return;
    const on = rule.days.includes(d);
    // At least one day stays on: none is the start date's anyway.
    if (on && rule.days.length === 1) return;
    edit({ days: on ? rule.days.filter((x) => x !== d) : WEEKDAYS.filter((x) => x === d || rule!.days.includes(x)) });
  }
  /** The monthly choices for the anchor day, and the rule's own when it is another. */
  const monthOptions = $derived.by(() => {
    const out: { key: string; label: string; change: Partial<CalendarRule> }[] = [];
    const add = (key: string, label: string, change: Partial<CalendarRule>) => {
      if (!out.some((o) => o.key === key)) out.push({ key, label, change });
    };
    if (rule?.month_day) add(`d${rule.month_day}`, `On day ${rule.month_day}`, { month_day: rule.month_day });
    if (rule?.nth && rule.weekday)
      add(`n${rule.nth}${rule.weekday}`, `On ${nthWords(rule.nth, rule.weekday)}`, {
        nth: rule.nth,
        weekday: rule.weekday,
      });
    if (months) {
      add(`d${months.day}`, `On day ${months.day}`, { month_day: months.day, nth: null, weekday: null });
      add(`n${months.nth}${months.weekday}`, `On ${nthWords(months.nth, months.weekday)}`, {
        nth: months.nth,
        weekday: months.weekday,
        month_day: null,
      });
      if (months.last)
        add(`n-1${months.weekday}`, `On ${nthWords(-1, months.weekday)}`, {
          nth: -1,
          weekday: months.weekday,
          month_day: null,
        });
    }
    return out;
  });
  const monthKey = $derived(rule?.nth && rule.weekday ? `n${rule.nth}${rule.weekday}` : `d${rule?.month_day ?? ''}`);
  const ends = $derived(rule?.until ? 'on' : rule?.count ? 'after' : 'never');
  const UNITS: { value: Freq; one: string; many: string }[] = [
    { value: 'daily', one: 'day', many: 'days' },
    { value: 'weekly', one: 'week', many: 'weeks' },
    { value: 'monthly', one: 'month', many: 'months' },
    { value: 'yearly', one: 'year', many: 'years' },
  ];
</script>

<svelte:window onpointerdown={away} onresize={() => open && close()} {onkeydown} />

<span class="iterfield">
  <button
    bind:this={button}
    {id}
    type="button"
    class="trigger"
    class:empty={!value.trim()}
    aria-haspopup="dialog"
    aria-expanded={open}
    aria-label={value.trim() ? `${label}: ${words}` : label}
    title={value.trim() ? value : label}
    onclick={() => (open ? close() : void show())}
  >
    <span class="text">{words || 'Set rule'}</span>
    <Repeat size={14} />
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
      <div class="modes" role="tablist" aria-label="How the rule is written">
        <button
          type="button"
          role="tab"
          data-first
          aria-selected={mode === 'taskwarrior'}
          class:on={mode === 'taskwarrior'}
          onclick={() => switchTo('taskwarrior')}>Taskwarrior</button
        >
        <button
          type="button"
          role="tab"
          aria-selected={mode === 'calendar'}
          class:on={mode === 'calendar'}
          title="A calendar rule (RFC 5545 RRULE, FREQ=…)"
          onclick={() => switchTo('calendar')}>Calendar (RRULE)</button
        >
      </div>

      {#if mode === 'taskwarrior'}
        <div class="words" role="group" aria-label="Periods">
          {#each PRESETS as p (p.value)}
            <button
              type="button"
              class="ghost word"
              class:on={value.trim() === p.value}
              onclick={() => (value = p.value)}>{p.label}</button
            >
          {/each}
        </div>
        <label class="small dim" for="iterfield-typed">Or as task takes it</label>
        <input
          id="iterfield-typed"
          class="mono"
          bind:value
          placeholder="3d, 2wk, P2W, every other day…"
          autocapitalize="off"
          autocomplete="off"
          spellcheck="false"
        />
      {:else if rule}
        <div class="group">
          <span class="small dim">Repeats every</span>
          <div class="row">
            <input
              class="count"
              type="number"
              min="1"
              aria-label="Every how many"
              value={rule.interval}
              oninput={(e) => {
                const n = Math.max(1, Math.floor(Number(e.currentTarget.value) || 1));
                edit({ interval: n });
              }}
            />
            <select aria-label="Unit" value={rule.freq} onchange={(e) => setFreq(e.currentTarget.value as Freq)}>
              {#each UNITS as u (u.value)}<option value={u.value}>{rule.interval === 1 ? u.one : u.many}</option>{/each}
            </select>
          </div>
        </div>
        {#if rule.freq === 'weekly'}
          <div class="group">
            <span class="small dim">Repeats on</span>
            <div class="days" role="group" aria-label="Weekdays">
              {#each WEEKDAYS as d, i (d)}
                <button
                  type="button"
                  class="day"
                  class:on={rule.days.includes(d)}
                  aria-pressed={rule.days.includes(d)}
                  aria-label={DAY_NAMES[i]}
                  title={DAY_NAMES[i]}
                  onclick={() => toggleDay(d)}>{DAY_NAMES[i][0]}</button
                >
              {/each}
            </div>
          </div>
        {:else if rule.freq === 'monthly' && monthOptions.length}
          <div class="group">
            <select
              aria-label="Which day of the month"
              value={monthKey}
              onchange={(e) => {
                const o = monthOptions.find((x) => x.key === e.currentTarget.value);
                if (o) edit(o.change);
              }}
            >
              {#each monthOptions as o (o.key)}<option value={o.key}>{o.label}</option>{/each}
            </select>
          </div>
        {/if}
        <fieldset class="group ends">
          <legend class="small dim">Ends</legend>
          <label class="end"
            ><input
              type="radio"
              name="iterfield-ends"
              checked={ends === 'never'}
              onchange={() => edit({ until: null, count: null })}
            /> Never</label
          >
          <label class="end"
            ><input
              type="radio"
              name="iterfield-ends"
              checked={ends === 'on'}
              onchange={() => edit({ until: anchor, count: null })}
            />
            On
            <input
              type="date"
              aria-label="Ends on"
              value={rule.until ?? ''}
              disabled={ends !== 'on'}
              oninput={(e) => e.currentTarget.value && edit({ until: e.currentTarget.value, count: null })}
            /></label
          >
          <label class="end"
            ><input
              type="radio"
              name="iterfield-ends"
              checked={ends === 'after'}
              onchange={() => edit({ count: 10, until: null })}
            />
            After
            <input
              class="count"
              type="number"
              min="1"
              aria-label="Ends after how many times"
              value={rule.count ?? 10}
              disabled={ends !== 'after'}
              oninput={(e) => edit({ count: Math.max(1, Math.floor(Number(e.currentTarget.value) || 1)), until: null })}
            /> times</label
          >
        </fieldset>
        <code class="small dim rrule" title="The rule as it is stored">{value}</code>
      {/if}

      {#if value.trim()}<p class="said">{words}</p>{/if}
      <div class="foot">
        <button
          type="button"
          class="ghost"
          disabled={!value.trim()}
          onclick={() => ((value = ''), (rule = null), (mode = 'taskwarrior'))}><X size={13} /> Clear</button
        >
        <button type="button" onclick={() => close(true)}>Done</button>
      </div>
    </div>
  {/if}
</span>

<style>
  .iterfield {
    display: flex;
  }
  .trigger {
    flex: 1;
    display: inline-flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 5px 9px;
    font-size: 13px;
    text-align: left;
  }
  .trigger.empty .text {
    color: var(--dim);
  }
  .panel {
    position: fixed;
    z-index: 40;
    width: min(340px, calc(100vw - 16px));
    box-sizing: border-box;
    display: grid;
    gap: 10px;
    padding: 10px;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    box-shadow: 0 6px 22px rgb(0 0 0 / 0.22);
  }
  .modes {
    display: grid;
    grid-template-columns: 1fr 1fr;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    overflow: hidden;
  }
  .modes button {
    border: 0;
    border-radius: 0;
    padding: 5px 8px;
    font-size: 12.5px;
    background: transparent;
  }
  .modes button.on {
    background: var(--accent);
    color: var(--accent-text);
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
  .group {
    display: grid;
    gap: 4px;
  }
  .row {
    display: flex;
    gap: 6px;
  }
  .count {
    width: 4.5em;
  }
  .days {
    display: flex;
    gap: 4px;
  }
  .day {
    width: 2.1em;
    height: 2.1em;
    padding: 0;
    border-radius: 999px;
    font-size: 12.5px;
  }
  .day.on {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-text);
  }
  .ends {
    margin: 0;
    padding: 0;
    border: 0;
  }
  .end {
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 2em;
    font-size: 13px;
  }
  .end input[type='date'] {
    flex: 1;
    min-width: 0;
  }
  .small {
    font-size: 12px;
  }
  .rrule {
    overflow-wrap: anywhere;
  }
  .said {
    margin: 0;
    font-size: 12.5px;
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
