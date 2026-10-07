<script lang="ts">
  import { fromParts } from './dates';
  import { X } from './icons';

  // `value` is '' | 'YYYY-MM-DD' | 'YYYY-MM-DDTHH:MM' (local time). The time is optional:
  // leave it empty for a whole day.
  let { value = $bindable(''), label = '', id = undefined }: { value?: string; label?: string; id?: string } = $props();

  const date = $derived(value.slice(0, 10));
  const time = $derived(value.length > 10 ? value.slice(11, 16) : '');

  const pad = (n: number) => String(n).padStart(2, '0');
  function dayOffset(days: number): string {
    const d = new Date();
    d.setDate(d.getDate() + days);
    return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
  }
</script>

<span class="dt" role="group" aria-label={label}>
  <input
    {id}
    type="date"
    value={date}
    aria-label={label ? `${label} date` : 'date'}
    oninput={(e) => (value = fromParts({ date: e.currentTarget.value, time }))}
  />
  <input
    type="time"
    value={time}
    disabled={!date}
    aria-label={label ? `${label} time (optional)` : 'time (optional)'}
    title="Optional. Leave empty for the whole day."
    oninput={(e) => (value = fromParts({ date, time: e.currentTarget.value }))}
  />
  <span class="presets">
    <button type="button" class="ghost" onclick={() => (value = fromParts({ date: dayOffset(0), time }))}>today</button>
    <button type="button" class="ghost" onclick={() => (value = fromParts({ date: dayOffset(1), time }))}>tmrw</button>
    <button type="button" class="ghost" onclick={() => (value = fromParts({ date: dayOffset(7), time }))}>+1w</button>
    {#if time}
      <button type="button" class="ghost" title="Make it a whole day" onclick={() => (value = date)}>no time</button>
    {/if}
    {#if value}
      <button type="button" class="ghost" aria-label="Clear {label}" onclick={() => (value = '')}><X size={13} /></button>
    {/if}
  </span>
</span>

<style>
  .dt { display: inline-flex; flex-wrap: wrap; align-items: center; gap: 4px; }
  input[type='date'] { width: 9.5em; }
  input[type='time'] { width: 6.5em; }
  .presets { display: inline-flex; flex-wrap: wrap; gap: 0; }
  @media (max-width: 760px) {
    .dt { width: 100%; }
    input[type='date'] { flex: 1 1 9.5em; }
    input[type='time'] { flex: 1 1 6.5em; }
    .presets { flex-basis: 100%; }
    .presets button { padding: 6px 10px; font-size: 13px; }
  }
  .presets button { padding: 2px 6px; font-size: 12px; color: var(--dim); display: inline-flex; align-items: center; }
</style>
