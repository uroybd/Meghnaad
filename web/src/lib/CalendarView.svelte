<script lang="ts">
  import { formatFor } from './dateformat';
  import { formatMoment } from './dates';
  import ReportTable from './ReportTable.svelte';
  import { store, type Entry } from './store.svelte';
  import type { CalendarDay, CalendarMonth, CalendarResult } from './types';

  /** `calendar`: months laid out the way `task calendar` prints them, drawn rather than typed. */
  let { result, entry = null }: { result: CalendarResult; entry?: Entry | null } = $props();

  const settings = $derived(store.config?.config.settings);
  const dateFmt = $derived(formatFor('report', settings));
  const pad = (n: number) => String(n).padStart(2, '0');

  // A click on a day with something due shows those tasks in the Tasks view, always in the built-in
  // `list` report (the report in focus might filter or order them differently, or not exist any more).
  function show(m: CalendarMonth, d: CalendarDay) {
    if (!d.due && !d.scheduled) return;
    const day = `${m.year}-${pad(m.month)}-${pad(d.day)}`;
    store.focusReport('list', [d.due ? `due:${day}` : `scheduled:${day}`]);
  }

  function describe(m: CalendarMonth, d: CalendarDay): string {
    const bits: string[] = [];
    if (d.today) bits.push('today');
    if (d.holiday) bits.push('holiday');
    if (d.due === 'overdue') bits.push('overdue');
    if (d.due === 'due-today') bits.push('due later today');
    if (d.due === 'due') bits.push('something is due');
    if (d.scheduled) bits.push('scheduled');
    return `${m.name} ${d.day}${bits.length ? ': ' + bits.join(', ') : ''}`;
  }
</script>

<div class="months">
  {#each result.months as m (m.year * 100 + m.month)}
    <section class="month" aria-label="{m.name} {m.year}">
      <h4>{m.name} {m.year}</h4>
      <table>
        <thead>
          <tr>
            {#if result.week_numbers}<th class="wk" aria-label="Week"></th>{/if}
            {#each result.weekdays as w}<th>{w}</th>{/each}
          </tr>
        </thead>
        <tbody>
          {#each m.weeks as week}
            <tr>
              {#if result.week_numbers}<td class="wk">{week.number ?? ''}</td>{/if}
              {#each week.days as d}
                {#if d}
                  {@const clickable = !!(d.due || d.scheduled)}
                  <td
                    class="day"
                    class:today={d.today}
                    class:weekend={d.weekend}
                    class:holiday={d.holiday}
                    class:scheduled={d.scheduled}
                    class:clickable
                    data-due={d.due}
                    title={describe(m, d)}
                  >
                    {#if clickable}
                      <button type="button" class="cell" onclick={() => show(m, d)} aria-label={describe(m, d)}>{d.day}</button>
                    {:else}
                      <span class="cell">{d.day}</span>
                    {/if}
                  </td>
                {:else}
                  <td class="empty"></td>
                {/if}
              {/each}
            </tr>
          {/each}
        </tbody>
      </table>
    </section>
  {/each}
</div>

{#if result.legend}
  <p class="legend dim" aria-label="Legend">
    <span class="key lg-today">today</span>
    <span class="key lg-weekend">weekend</span>
    {#if result.due_colours}
      <span class="key lg-due">due</span>
      <span class="key lg-duetoday">due today</span>
      <span class="key lg-overdue">overdue</span>
      <span class="key lg-scheduled">scheduled</span>
    {/if}
    {#if result.holiday_colours}<span class="key lg-holiday">holiday</span>{/if}
    {#if result.week_numbers}<span class="key lg-week">week number</span>{/if}
  </p>
{/if}

{#if result.holidays}
  <h4 class="sub">Holidays</h4>
  {#if result.holidays.length === 0}
    <p class="dim">No holidays in these months.</p>
  {:else}
    <table class="holidays">
      <thead><tr><th>Date</th><th>Holiday</th></tr></thead>
      <tbody>
        {#each result.holidays as h}
          <tr><td>{formatMoment(h.date, undefined, dateFmt)}</td><td>{h.name}</td></tr>
        {/each}
      </tbody>
    </table>
  {/if}
{/if}

{#if result.details}
  <h4 class="sub">Due in these months</h4>
  {#if result.details.rows.length === 0}
    <p class="dim">Nothing is due in these months.</p>
  {:else}
    <ReportTable result={result.details} {entry} />
  {/if}
{/if}

<style>
  .months { display: grid; grid-template-columns: repeat(auto-fill, minmax(17.5rem, 1fr)); gap: 18px 24px; margin: 8px 0 12px; }
  h4 { margin: 0 0 6px; text-align: center; font-size: 15px; }
  h4.sub { text-align: left; margin: 16px 0 6px; }
  table { border-collapse: collapse; width: 100%; table-layout: fixed; }
  th { color: var(--dim); font-weight: 500; font-size: 12px; padding: 2px 0 4px; text-align: center; }
  td { padding: 1px; text-align: center; }
  .wk { width: 2.2em; color: var(--dim); font-size: 11.5px; font-variant-numeric: tabular-nums; }
  .cell {
    display: block; width: 100%; padding: 4px 0; border-radius: 6px; border: 0; background: transparent;
    font: inherit; font-variant-numeric: tabular-nums; color: inherit; line-height: 1.3;
  }
  button.cell { cursor: pointer; }
  button.cell:hover { outline: 1px solid var(--accent); }
  .weekend .cell { color: var(--dim); }
  .holiday .cell { background: color-mix(in srgb, var(--warn) 22%, transparent); color: var(--text); }
  .scheduled .cell { box-shadow: inset 0 -3px 0 hsl(25 var(--seg-s) var(--seg-l)); }
  td[data-due='due'] .cell { background: color-mix(in srgb, var(--ok) 24%, transparent); color: var(--text); font-weight: 600; }
  td[data-due='due-today'] .cell { background: color-mix(in srgb, hsl(290 var(--seg-s) var(--seg-l)) 26%, transparent); color: var(--text); font-weight: 600; }
  td[data-due='overdue'] .cell { background: color-mix(in srgb, var(--err) 26%, transparent); color: var(--text); font-weight: 600; }
  .today .cell { outline: 2px solid var(--accent); outline-offset: -2px; font-weight: 700; }

  .legend { display: flex; flex-wrap: wrap; gap: 6px 14px; font-size: 12.5px; margin: 6px 0 0; }
  .key { display: inline-flex; align-items: center; gap: 6px; }
  .key::before { content: ''; width: 14px; height: 14px; border-radius: 4px; border: 1px solid var(--line); }
  .lg-today::before { outline: 2px solid var(--accent); outline-offset: -2px; border: 0; }
  .lg-weekend::before { background: var(--panel-2); }
  .lg-due::before { background: color-mix(in srgb, var(--ok) 24%, transparent); }
  .lg-duetoday::before { background: color-mix(in srgb, hsl(290 var(--seg-s) var(--seg-l)) 26%, transparent); }
  .lg-overdue::before { background: color-mix(in srgb, var(--err) 26%, transparent); }
  .lg-scheduled::before { box-shadow: inset 0 -3px 0 hsl(25 var(--seg-s) var(--seg-l)); }
  .lg-holiday::before { background: color-mix(in srgb, var(--warn) 22%, transparent); }
  .lg-week::before { display: none; }
  .holidays { max-width: 520px; table-layout: auto; }
  .holidays th, .holidays td { text-align: left; padding: 3px 18px 3px 0; }
</style>
