<script lang="ts">
  import { formatMoment, formatSeconds, formatStamp } from './dates';
  import { udaLabel } from './format';
  import { Lock, Pencil, Timer } from './icons';
  import { visibleNotes } from './journal';
  import { store } from './store.svelte';
  import type { Row } from './types';

  let {
    task,
    onedit,
    onopen,
    embedded = false,
  }: {
    task: Row;
    onedit?: (row: Row) => void;
    /** Open another task (a dependency) when its id is clicked. */
    onopen?: (uuid: string) => void;
    /** Inside the drawer: no card chrome or title (the drawer has its own). */
    embedded?: boolean;
  } = $props();

  const defs = $derived(store.config?.config.udas ?? {});
  const udaKeys = $derived(Object.keys(task.extra).filter((k) => k in defs).sort());
  const dates: [string, number | null][] = $derived([
    ['Entered', task.entry], ['Due', task.due], ['Scheduled', task.scheduled], ['Wait', task.wait],
    ['Until', task.until], ['Started', task.start], ['Ended', task.end], ['Modified', task.modified],
  ]);
  // The journal's "Started task"/"Stopped task" notes are bookkeeping; the sessions table shows them.
  const notes = $derived(visibleNotes(task.annotations, store.config?.journal));
  const totalTracked = $derived(task.active_seconds ?? task.sessions.reduce((n, s) => n + s.seconds, 0));
  const depTitle = (uuid: string) => store.tasks.find((t) => t.uuid === uuid)?.description ?? uuid;
</script>

<article class:card={!embedded}>
  {#if !embedded}
    <header class="row">
      <strong class="grow">{task.description}</strong>
      {#if onedit}<button class="ghost btn" onclick={() => onedit(task)}><Pencil size={14} /> Edit</button>{/if}
    </header>
  {/if}
  <dl>
    <dt>ID</dt><dd>{task.id ?? '–'} <span class="dim mono">{task.uuid}</span></dd>
    <dt>Status</dt><dd>{task.status}{#if task.virtual_tags.includes('WAITING')} <span class="chip">waiting</span>{/if}{#if task.blocked} <span class="chip">blocked</span>{/if}{#if task.blocking} <span class="chip">blocking</span>{/if}</dd>
    {#if task.project}<dt>Project</dt><dd>{task.project}</dd>{/if}
    {#if task.priority}<dt>Priority</dt><dd>{task.priority}</dd>{/if}
    {#if task.tags.length}<dt>Tags</dt><dd>{#each task.tags as t}<span class="chip">{t}</span> {/each}</dd>{/if}
    {#each dates as [label, ts]}
      {#if ts != null}<dt>{label}</dt><dd>{formatMoment(ts)}</dd>{/if}
    {/each}
    {#if task.active_seconds != null}
      <dt>Time tracked</dt>
      <dd data-testid="tracked">{formatSeconds(task.active_seconds)}{#if task.start != null} <span class="chip live"><Timer size={11} /> running</span>{/if}</dd>
    {/if}
    {#if task.depends.length}
      <dt>Depends on</dt>
      <dd>
        {#each task.depends as d}
          {#if onopen}
            <button class="ghost dep" title={depTitle(d)} onclick={() => onopen(d)}>{depTitle(d)} <span class="dim mono">{d.slice(0, 8)}</span></button>
          {:else}
            <span class="mono">{d.slice(0, 8)}</span>
          {/if}
        {/each}
      </dd>
    {/if}
    {#each udaKeys as k}
      <dt>{udaLabel(defs[k])}</dt><dd>{task.extra[k]}</dd>
    {/each}
    <dt>Urgency</dt><dd>{task.urgency.toFixed(2)}</dd>
    {#if task.virtual_tags.length}
      <dt>Virtual tags</dt><dd class="dim">{task.virtual_tags.join(' ')}</dd>
    {/if}
  </dl>

  {#if task.sessions.length}
    <h4 class="sess"><Timer size={13} /> Time sessions</h4>
    <table class="sessions" data-testid="sessions">
      <thead>
        <tr><th>#</th><th>Started</th><th>Stopped</th><th class="num">Duration</th></tr>
      </thead>
      <tbody>
        {#each task.sessions as s, i}
          <tr class:running={s.end == null}>
            <td class="dim">{i + 1}</td>
            <td class="mono">{formatStamp(s.start)}</td>
            <td class="mono">
              {#if s.end != null}{formatStamp(s.end)}{:else}<span class="chip live"><Timer size={11} /> running</span>{/if}
            </td>
            <td class="num mono">{formatSeconds(s.seconds)}</td>
          </tr>
        {/each}
      </tbody>
      <tfoot>
        <tr><td colspan="3">Total</td><td class="num mono" data-testid="sessions-total">{formatSeconds(totalTracked)}</td></tr>
      </tfoot>
    </table>
  {/if}

  {#if notes.length}
    <h4>Annotations</h4>
    <ul>
      {#each notes as a}<li><span class="dim">{formatMoment(a.entry)}</span> {a.text}</li>{/each}
    </ul>
  {/if}

  {#if task.orphans.length}
    <h4 title="These properties exist on the task but aren't defined as UDAs in your taskrc.">
      <Lock size={13} /> Not defined in your taskrc <span class="chip">read-only</span>
    </h4>
    <dl class="orphans">
      {#each task.orphans as k}
        <dt class="mono">{k}</dt><dd class="mono">{task.extra[k]}</dd>
      {/each}
    </dl>
  {/if}
</article>

<style>
  .card { border: 1px solid var(--line); border-radius: var(--radius); background: var(--panel); padding: 10px 14px; margin: 6px 0; }
  dl { display: grid; grid-template-columns: max-content 1fr; gap: 2px 14px; margin: 8px 0; }
  dt { color: var(--dim); }
  dd { margin: 0; min-width: 0; overflow-wrap: anywhere; }
  h4 { margin: 10px 0 2px; font-size: 13px; }
  ul { margin: 0; padding-left: 18px; }
  .orphans { opacity: 0.85; }
  .live { display: inline-flex; align-items: center; gap: 3px; color: var(--ok); border-color: var(--ok); }
  h4 { display: flex; align-items: center; gap: 5px; }
  .btn { display: inline-flex; align-items: center; gap: 5px; }
  .sess { display: flex; align-items: center; gap: 5px; }
  .sessions { width: 100%; border-collapse: collapse; font-size: 13px; margin: 2px 0 6px; }
  .sessions th, .sessions td { text-align: left; padding: 2px 10px 2px 0; white-space: nowrap; }
  .sessions th { color: var(--dim); font-weight: 500; font-size: 12px; border-bottom: 1px solid var(--line); }
  .sessions .num { text-align: right; padding-right: 0; font-variant-numeric: tabular-nums; }
  .sessions tfoot td { border-top: 1px solid var(--line); font-weight: 600; padding-top: 3px; }
  .sessions tr.running td { color: var(--ok); }
  .dep { display: block; padding: 0 4px; margin-left: -4px; text-align: left; }
</style>
