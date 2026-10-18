<script lang="ts">
  import { tzOffsetSeconds } from './api';
  import {
    SECTION_TYPES,
    describeBy,
    describeMeasure,
    describeSpan,
    formatValue,
    orderChart,
    parseTasks,
    seriesOf,
    stampEpoch,
    type ExportedTask,
    type Section,
    type SectionData,
  } from './boards';
  import { formatFor } from './dateformat';
  import { compactDuration, formatMoment } from './dates';
  import { Search, X } from './icons';
  import { store } from './store.svelte';

  /**
   * What a board section is made of, for whoever wants to check it: the filter it ran, the tasks that filter got, and what was
   * worked out of them (the figure, or each label on the chart's axis with each series' value there).
   */
  let { section, data, onclose }: { section: Section; data: SectionData; onclose: () => void } = $props();

  let dialog: HTMLDialogElement | undefined = $state();
  $effect(() => {
    if (dialog && !dialog.open) dialog.showModal();
  });

  /** The most tasks listed for one filter; the count says how many there are. */
  const LIMIT = 200;

  const typeLabel = $derived(SECTION_TYPES.find((t) => t.id === section.type)?.label ?? section.type);
  const sources = $derived(data.kind === 'answer' || data.kind === 'chart' ? data.sources : []);
  const listed = $derived(sources.map((src) => ({ ...src, tasks: parseTasks(src.text) })));
  const answer = $derived(data.kind === 'answer' ? data.answer : null);
  const chart = $derived(data.kind === 'chart' ? orderChart(data.chart, section) : null);
  const unit = $derived(answer?.unit ?? chart?.unit ?? 'number');
  const fmt = (v: number | null) => formatValue(v, unit, compactDuration);
  const matched = $derived(data.kind === 'table' && data.result.kind === 'report' ? data.result.matched : null);

  // The columns of the lists of tasks: what identifies a task, and whatever the section measures or groups by.
  const BASE = ['id', 'description', 'project', 'status', 'tags'];
  const columns = $derived.by(() => {
    const extra = [
      ...seriesOf(section).map((x) => (x.measure ?? section.measure).field),
      section.by?.field,
      section.by?.time?.field,
    ];
    return [...new Set([...BASE, ...extra.filter((f): f is string => !!f)])];
  });

  const dateFmt = $derived(formatFor('report', store.config?.config.settings));
  function cell(task: ExportedTask, column: string): string {
    const v = task[column];
    if (v === undefined || v === null || v === '' || (column === 'id' && v === 0)) return '–';
    const epoch = stampEpoch(v);
    if (epoch !== null) return formatMoment(epoch, tzOffsetSeconds(), dateFmt);
    if (Array.isArray(v)) return v.join(', ');
    return typeof v === 'object' ? JSON.stringify(v) : String(v);
  }

  /** Look at these tasks on the Tasks page, with this filter. */
  function open(filter: string) {
    store.filter = filter;
    store.view = 'tasks';
    dialog?.close();
  }

  /** Look at one of these tasks: close this panel (it covers the page) and open the task's drawer. */
  function openTask(task: ExportedTask) {
    const uuid = task.uuid;
    if (typeof uuid !== 'string') return;
    dialog?.close();
    store.openDetail(uuid, null);
  }
</script>

<dialog bind:this={dialog} {onclose} aria-labelledby="info-title">
  <header>
    <h3 id="info-title">{section.title} <span class="dim">· {typeLabel}</span></h3>
    <button class="ghost" aria-label="Close" title="Close" onclick={() => dialog?.close()}><X size={16} /></button>
  </header>

  <h4>What it shows</h4>
  <dl>
    {#if section.type === 'table'}
      <dt>Report</dt>
      <dd><code>{section.report || 'next'}</code></dd>
    {:else}
      <dt>Measure</dt>
      <dd>{describeMeasure(section.measure)}{section.series?.length ? ' (unless a series says otherwise)' : ''}</dd>
    {/if}
    {#if describeBy(section.by)}
      <dt>Grouped by</dt>
      <dd>{describeBy(section.by)}</dd>
    {/if}
    {#if chart}
      <dt>Order</dt>
      <dd>
        {section.sort ? section.sort : section.by?.time ? 'in time order' : 'largest first'}{section.limit
          ? `, the first ${section.limit}`
          : ''}{section.stacked ? ', stacked' : ''}
      </dd>
    {/if}
  </dl>

  {#if data.kind === 'error'}
    <h4>Problem</h4>
    <p class="err" role="alert">{data.message}</p>
  {/if}

  {#if data.kind !== 'error'}
    <h4>Filter</h4>
  {/if}
  {#if data.kind === 'table'}
    <p class="src">
      <code>{data.filter || '(every task)'}</code>
      <button onclick={() => open(data.filter)}><Search size={13} /> Open in Tasks</button>
    </p>
    {#if matched !== null}<p>The report matched <strong>{matched}</strong> {matched === 1 ? 'task' : 'tasks'}.</p>{/if}
  {:else}
    <ul class="sources">
      {#each sources as src, i (i)}
        <li class="src">
          {#if sources.length > 1}<strong>{src.label}</strong>{/if}
          <code>{src.filter || '(every task)'}</code>
          <button onclick={() => open(src.filter)}><Search size={13} /> Open in Tasks</button>
          {#if seriesOf(section)[i]?.span}
            <span class="dim small">{describeSpan(seriesOf(section)[i].span!)}</span>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}

  {#if answer}
    <h4>Result</h4>
    <p>
      <strong class="big">{fmt(answer.total)}</strong>
      from {answer.tasks}
      {answer.tasks === 1 ? 'task' : 'tasks'}.
    </p>
  {:else if chart}
    <h4>Axis values</h4>
    {#if !chart.labels.length}
      <p class="dim">Nothing on the axis.</p>
    {:else}
      <div class="scroll">
        <table>
          <thead>
            <tr>
              <th>{describeBy(section.by) ?? 'Label'}</th>
              {#each chart.datasets as d, i (i)}<th class="num">{d.label}</th>{/each}
            </tr>
          </thead>
          <tbody>
            {#each chart.labels as l, i (l.key)}
              <tr>
                <td>{l.label}</td>
                {#each chart.datasets as d, j (j)}
                  <td class="num">{fmt(d.data[i])}<span class="dim"> ({d.counts[i]})</span></td>
                {/each}
              </tr>
            {/each}
          </tbody>
          <tfoot>
            <tr>
              <th>Over all the tasks</th>
              {#each chart.datasets as d, j (j)}
                <td class="num">{fmt(d.total)}<span class="dim"> ({d.tasks})</span></td>
              {/each}
            </tr>
          </tfoot>
        </table>
      </div>
      <p class="dim small">
        Each value is the measure of that label’s tasks; the number in brackets is how many tasks they are.
      </p>
    {/if}
  {/if}

  {#if listed.length}
    <h4>Tasks the filter got</h4>
    {#each listed as src, i (i)}
      {#if listed.length > 1}<h5>{src.label}</h5>{/if}
      <p class="dim small">{src.tasks.length} {src.tasks.length === 1 ? 'task' : 'tasks'}</p>
      {#if src.tasks.length}
        <div class="scroll tasks">
          <table>
            <thead>
              <tr
                >{#each columns as c (c)}<th>{c}</th>{/each}</tr
              >
            </thead>
            <tbody>
              {#each src.tasks.slice(0, LIMIT) as t, k (k)}
                <tr
                  >{#each columns as c (c)}<td
                      >{#if c === 'description' && typeof t.uuid === 'string'}<button
                          type="button"
                          class="link"
                          title="Open this task"
                          onclick={() => openTask(t)}>{cell(t, c)}</button
                        >{:else}{cell(t, c)}{/if}</td
                    >{/each}</tr
                >
              {/each}
            </tbody>
          </table>
        </div>
        {#if src.tasks.length > LIMIT}
          <p class="dim small">and {src.tasks.length - LIMIT} more (open them in Tasks to see all).</p>
        {/if}
      {/if}
    {/each}
  {/if}
</dialog>

<style>
  dialog {
    width: min(900px, calc(100vw - 24px));
    max-height: calc(100vh - 32px);
    padding: 16px 18px;
    background: var(--panel);
    color: var(--text);
    border: 1px solid var(--line);
    border-radius: var(--radius);
  }
  dialog::backdrop {
    background: rgb(0 0 0 / 0.4);
  }
  header {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  h3 {
    flex: 1;
    margin: 0;
    font-size: 17px;
  }
  h4 {
    margin: 16px 0 6px;
    font-size: 13px;
    color: var(--dim);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  h5 {
    margin: 10px 0 0;
    font-size: 13px;
  }
  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 3px 14px;
    margin: 0;
  }
  dt {
    color: var(--dim);
  }
  dd {
    margin: 0;
  }
  .sources {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 6px;
  }
  .src {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin: 0;
  }
  code {
    font-family: var(--mono);
    font-size: 12.5px;
    background: var(--panel-2);
    padding: 1px 6px;
    border-radius: 4px;
    overflow-wrap: anywhere;
  }
  .big {
    font-size: 22px;
  }
  .small {
    font-size: 12px;
    margin: 4px 0;
  }
  .scroll {
    overflow: auto;
    max-width: 100%;
  }
  .tasks {
    max-height: 280px;
  }
  table {
    border-collapse: collapse;
    font-size: 13px;
    min-width: 100%;
  }
  th,
  td {
    text-align: left;
    padding: 3px 10px 3px 0;
    border-bottom: 1px solid var(--line);
    white-space: nowrap;
  }
  td:nth-child(2) {
    white-space: normal;
  }
  .link {
    all: unset;
    cursor: pointer;
    color: var(--accent, var(--text));
    text-decoration: underline;
    text-decoration-color: var(--line);
    text-underline-offset: 2px;
  }
  .link:hover,
  .link:focus-visible {
    text-decoration-color: currentColor;
  }
  .link:focus-visible {
    outline: 2px solid var(--accent, currentColor);
    outline-offset: 2px;
  }
  th {
    color: var(--dim);
    font-weight: 600;
    position: sticky;
    top: 0;
    background: var(--panel);
  }
  .num {
    text-align: right;
  }
  tfoot th,
  tfoot td {
    border-top: 2px solid var(--line);
    border-bottom: 0;
  }
</style>
