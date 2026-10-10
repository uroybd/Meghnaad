<script lang="ts">
  import { runCli } from './api';
  import { formatFor } from './dateformat';
  import { formatMoment, formatSeconds, formatStamp } from './dates';
  import { describeChange } from './history';
  import { projectSegments, shortUuid, udaLabel, urgencyLevel } from './format';
  import { CornerUpLeft, Lock, Pencil, Repeat, Timer } from './icons';
  import { visibleNotes } from './journal';
  import ProjectPath from './ProjectPath.svelte';
  import TagChip from './TagChip.svelte';
  import { describeRecur } from './recurrence';
  import StatusPill, { type Kind } from './StatusPill.svelte';
  import DepItem from './DepItem.svelte';
  import DepLink from './DepLink.svelte';
  import UuidTip from './UuidTip.svelte';
  import { store } from './store.svelte';
  import type { HistoryEntry, Row } from './types';

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

  // The history is not part of the task: it is fetched when the section is opened, and again when the task changes.
  let histOpen = $state(false);
  let history = $state<HistoryEntry[] | null>(null);
  let histError = $state<string | null>(null);
  let asked = 0;
  $effect(() => {
    void task.uuid;
    void task.modified;
    if (!histOpen) return;
    const mine = ++asked;
    histError = null;
    runCli({ args: ['_history', task.uuid] })
      .then(({ result }) => {
        if (mine !== asked) return;
        if (result.kind === 'json') history = result.value as HistoryEntry[];
        else histError = result.kind === 'error' ? result.message : 'The history could not be read.';
      })
      .catch((e) => {
        if (mine === asked) histError = e instanceof Error ? e.message : String(e);
      });
  });
  // Another task: forget the last one's history, so it is never shown against the wrong task.
  $effect(() => {
    void task.uuid;
    history = null;
  });

  const defs = $derived(store.config?.config.udas ?? {});
  const infoFmt = $derived(formatFor('info', store.config?.config.settings));
  const noteFmt = $derived(formatFor('infoNote', store.config?.config.settings));
  // History moments carry seconds (a row is the changes within one second); a configured
  // `dateformat.info` wins, as in Taskwarrior.
  const stamp = (epoch: number) => (infoFmt ? formatMoment(epoch, undefined, infoFmt) : formatStamp(epoch));
  const waiting = $derived(task.status === 'pending' && task.waiting === true);
  const udaKeys = $derived(
    Object.keys(task.extra)
      .filter((k) => k in defs)
      .sort(),
  );
  const dates: [string, number | null][] = $derived([
    ['Entered', task.entry],
    ['Due', task.due],
    ['Scheduled', task.scheduled],
    ['Wait', task.wait],
    ['Until', task.until],
    ['Started', task.start],
    ['Ended', task.end],
    ['Modified', task.modified],
  ]);
  // The journal's "Started task"/"Stopped task" notes are bookkeeping; the sessions table shows them.
  const notes = $derived(visibleNotes(task.annotations, store.config?.journal));
  const totalTracked = $derived(task.active_seconds ?? task.sessions.reduce((n, s) => n + s.seconds, 0));
  // A recurring template lists its instances (newest first); an instance links back to it.
  let instances = $state<Row[]>([]);
  $effect(() => {
    const uuid = task.uuid;
    void task.mask; // reload when an instance is added or finished
    if (task.status !== 'recurring') {
      instances = [];
      return;
    }
    let live = true;
    runCli({ args: [`parent:${uuid}`, 'all'] })
      .then((res) => {
        if (live && res.result.kind === 'report') instances = res.result.rows;
      })
      .catch(() => {});
    return () => (live = false);
  });
  const shownInstances = $derived([...instances].sort((a, b) => (b.due ?? 0) - (a.due ?? 0)).slice(0, 10));
  // The tasks waiting on this one are looked for when asked, not with every task shown.
  let blockers = $state<Row[] | null>(null);
  let askingBlockers = $state(false);
  let blockersError = $state<string | null>(null);
  // Another task, or a change to the tasks: what was found no longer holds.
  $effect(() => {
    void task.uuid;
    void store.rev;
    blockers = null;
    blockersError = null;
  });
  async function loadBlockers() {
    askingBlockers = true;
    blockersError = null;
    try {
      const { result } = await runCli({ args: ['status:pending', `depends.has:${task.uuid}`, 'all'] });
      if (result.kind === 'report') blockers = result.rows;
      else blockersError = result.kind === 'error' ? result.message : 'The tasks could not be read.';
    } catch (e) {
      blockersError = e instanceof Error ? e.message : String(e);
    } finally {
      askingBlockers = false;
    }
  }
  const parentTitle = $derived(task.parent ? store.tasks.find((t) => t.uuid === task.parent)?.description : undefined);
</script>

<article class:card={!embedded}>
  {#if !embedded}
    <header class="row">
      <strong class="grow">{task.description}</strong>
      {#if onedit}<button class="ghost btn" onclick={() => onedit(task)}><Pencil size={14} /> Edit</button>{/if}
    </header>
  {/if}
  <dl>
    <dt>ID</dt>
    <dd>
      <span class:idnum={task.id != null}
        ><UuidTip text={task.id != null ? String(task.id) : '–'} uuid={task.uuid} /></span
      ><code class="uuid" title={task.uuid}>{shortUuid(task.uuid)}</code>
    </dd>
    <dt>Status</dt>
    <dd class="pills">
      {#if waiting}<StatusPill kind="waiting" />{:else}<StatusPill
          kind={task.status as Kind}
          label={task.status}
        />{/if}
      {#if task.blocked}<StatusPill kind="blocked" />{/if}
      {#if task.blocking}<StatusPill kind="blocking" />{/if}
    </dd>
    {#if task.recur}
      <dt>Repeats</dt>
      <dd data-testid="repeats">
        <span class="chip"><Repeat size={11} /> {describeRecur(task.recur)}</span>
        {#if task.status === 'recurring'}<span class="chip">template</span>{/if}
        {#if task.parent}
          {#if onopen}
            <button class="ghost dep inline" title="Open the recurring task" onclick={() => onopen(task.parent!)}>
              <CornerUpLeft size={12} />
              {parentTitle ?? task.parent.slice(0, 8)}
            </button>
          {:else}
            <span class="dim">of</span> <DepLink uuid={task.parent} />
          {/if}
        {/if}
      </dd>
    {/if}
    {#if task.project}<dt>Project</dt>
      <dd><ProjectPath segments={projectSegments(task.project)} onpick={(p) => store.showProject(p)} /></dd>{/if}
    {#if task.priority}<dt>Priority</dt>
      <dd class="pri-{task.priority.toLowerCase()}">{task.priority}</dd>{/if}
    {#if task.tags.length}<dt>Tags</dt>
      <dd>
        {#each task.tags as t (t)}<TagChip tag={t} />{/each}
      </dd>{/if}
    {#each dates as [label, ts], _i (_i)}
      {#if ts != null}<dt>{label}</dt>
        <dd class:overdue={label === 'Due' && task.virtual_tags?.includes('OVERDUE')}>
          {formatMoment(ts, undefined, infoFmt)}
        </dd>{/if}
    {/each}
    {#if task.active_seconds != null}
      <dt>Time tracked</dt>
      <dd data-testid="tracked">
        {formatSeconds(task.active_seconds)}{#if task.start != null}
          <span class="chip live"><Timer size={11} /> running</span>{/if}
      </dd>
    {/if}
    {#if task.depends.length}
      <dt>Depends on</dt>
      <dd>
        {#each task.depends as d (d)}<DepItem uuid={d} {onopen} />{/each}
      </dd>
    {/if}
    {#if task.blocking}
      <dt>Blocking</dt>
      <dd>
        {#if blockers === null}
          <button class="ghost ask" disabled={askingBlockers} onclick={loadBlockers}
            >{askingBlockers ? 'Loading…' : 'Show the tasks waiting on this'}</button
          >
          {#if blockersError}<span class="err" role="alert">{blockersError}</span>{/if}
        {:else}
          {#each blockers as b (b.uuid)}<DepItem uuid={b.uuid} row={b} {onopen} />{:else}<span class="dim"
              >Nothing is waiting on it now.</span
            >{/each}
        {/if}
      </dd>
    {/if}
    {#each udaKeys as k (k)}
      <dt>{udaLabel(defs[k])}</dt>
      <dd class="pre">{task.extra[k]}</dd>
    {/each}
    <dt>Urgency</dt>
    <dd class="urg-{urgencyLevel(task.urgency)}">{task.urgency.toFixed(2)}</dd>
    {#if task.virtual_tags?.length}
      <dt>Virtual tags</dt>
      <dd class="dim">{task.virtual_tags?.join(' ')}</dd>
    {/if}
  </dl>

  {#if task.status === 'recurring' && shownInstances.length}
    <h4 class="sess"><Repeat size={13} /> Instances</h4>
    <ul class="instances" data-testid="instances">
      {#each shownInstances as i (i.uuid)}
        <li>
          {#if onopen}
            <button class="ghost dep inline" onclick={() => onopen(i.uuid)}>
              {i.due != null ? formatMoment(i.due, undefined, infoFmt) : 'no due date'}
            </button>
          {:else}
            {i.due != null ? formatMoment(i.due, undefined, infoFmt) : 'no due date'}
          {/if}
          <span class="chip">{i.status}</span>
        </li>
      {/each}
    </ul>
    {#if instances.length > shownInstances.length}
      <p class="dim more">+{instances.length - shownInstances.length} older</p>
    {/if}
  {/if}

  {#if task.sessions.length}
    <h4 class="sess"><Timer size={13} /> Time sessions</h4>
    <div class="scroll">
      <table class="sessions" data-testid="sessions">
        <thead>
          <tr><th>#</th><th>Started</th><th>Stopped</th><th class="num">Duration</th></tr>
        </thead>
        <tbody>
          {#each task.sessions as s, i (i)}
            <tr class:running={s.end == null}>
              <td class="dim">{i + 1}</td>
              <td class="mono">{formatStamp(s.start)}</td>
              <td class="mono">
                {#if s.end != null}{formatStamp(s.end)}{:else}<span class="chip live"><Timer size={11} /> running</span
                  >{/if}
              </td>
              <td class="num mono">{formatSeconds(s.seconds)}</td>
            </tr>
          {/each}
        </tbody>
        <tfoot>
          <tr
            ><td colspan="3">Total</td><td class="num mono" data-testid="sessions-total"
              >{formatSeconds(totalTracked)}</td
            ></tr
          >
        </tfoot>
      </table>
    </div>
  {/if}

  {#if notes.length}
    <h4>Annotations</h4>
    <ul>
      {#each notes as a, _i (_i)}<li>
          <span class="dim">{formatMoment(a.entry, undefined, noteFmt)}</span> <span class="pre">{a.text}</span>
        </li>{/each}
    </ul>
  {/if}

  <!-- Closed until asked for, and fetched then (`_history`): the log grows with every edit, and is rarely what you opened the task for. -->
  <details class="hist" bind:open={histOpen}>
    <summary title="What changed and when, from the task's operation log (journal.info).">
      History{#if history}
        <span class="dim">· {history.length} {history.length === 1 ? 'moment' : 'moments'}</span>{/if}
    </summary>
    {#if histError}
      <p class="err" role="alert">{histError}</p>
    {:else if history === null}
      <p class="dim">Loading…</p>
    {:else if !history.length}
      <p class="dim">
        Nothing is recorded: <code>journal.info</code> is off, or the changes came before this app last started.
      </p>
    {:else}
      <div class="scroll">
        <table class="history" data-testid="history">
          <thead><tr><th>Date</th><th>Modification</th></tr></thead>
          <tbody>
            {#each history as e, _i (_i)}
              <tr>
                <td class="mono">{stamp(e.at)}</td>
                <td
                  >{#each e.changes as c, _i (_i)}<div class="pre">{describeChange(c, stamp)}</div>{/each}</td
                >
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </details>

  {#if task.orphans.length}
    <h4 title="These properties exist on the task but aren't defined as UDAs in your taskrc.">
      <Lock size={13} /> Not defined in your taskrc <span class="chip">read-only</span>
    </h4>
    <dl class="orphans">
      {#each task.orphans as k (k)}
        <dt class="mono">{k}</dt>
        <dd class="mono pre">{task.extra[k]}</dd>
      {/each}
    </dl>
  {/if}
</article>

<style>
  .card {
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--panel);
    padding: 16px 20px;
    margin: 12px 0;
  }
  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 5px 20px;
    margin: 12px 0;
  }
  dt {
    color: var(--dim);
  }
  dd {
    margin: 0;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  h4 {
    margin: 16px 0 6px;
    font-size: 14px;
  }
  ul {
    margin: 0;
    padding-left: 18px;
  }
  .orphans {
    opacity: 0.85;
  }
  .live {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    color: var(--ok);
    border-color: var(--ok);
  }
  h4 {
    display: flex;
    align-items: center;
    gap: 5px;
  }
  .btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  .sess {
    display: flex;
    align-items: center;
    gap: 5px;
  }
  .scroll {
    overflow-x: auto;
  }
  .sessions {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
    margin: 2px 0 6px;
  }
  .sessions th,
  .sessions td {
    text-align: left;
    padding: 2px 10px 2px 0;
    white-space: nowrap;
  }
  .sessions th {
    color: var(--dim);
    font-weight: 500;
    font-size: 12px;
    border-bottom: 1px solid var(--line);
  }
  .sessions .num {
    text-align: right;
    padding-right: 0;
    font-variant-numeric: tabular-nums;
  }
  .sessions tfoot td {
    border-top: 1px solid var(--line);
    font-weight: 600;
    padding-top: 3px;
  }
  .sessions tr.running td {
    color: var(--ok);
  }
  .hist .err {
    color: var(--err);
  }
  .hist {
    margin: 16px 0 6px;
  }
  .hist summary {
    cursor: pointer;
    font-size: 14px;
    font-weight: 600;
  }
  .hist summary .dim {
    font-weight: 400;
    font-size: 13px;
  }
  .hist[open] summary {
    margin-bottom: 6px;
  }
  .history {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
    margin: 2px 0 6px;
  }
  .history th,
  .history td {
    text-align: left;
    padding: 3px 12px 3px 0;
    vertical-align: top;
  }
  .history td:first-child {
    white-space: nowrap;
    color: var(--dim);
  }
  .history th {
    color: var(--dim);
    font-weight: 500;
    font-size: 12px;
    border-bottom: 1px solid var(--line);
  }
  .history tbody td {
    border-bottom: 1px solid color-mix(in srgb, var(--line) 60%, transparent);
  }
  .instances {
    list-style: none;
    padding: 0;
    margin: 2px 0;
  }
  .instances li {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .more {
    margin: 2px 0;
    font-size: 12px;
  }
  .ask {
    padding: 0 4px;
    margin-left: -4px;
    color: var(--accent);
  }
  .dep.inline {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .dep {
    display: block;
    padding: 0 4px;
    margin-left: -4px;
    text-align: left;
  }
  .pre {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .pills {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    align-items: center;
  }
  /* The uuid reads like inline code in markdown: its own tinted box, apart from the id. */
  .uuid {
    margin-left: 12px;
    padding: 1px 7px;
    border-radius: 5px;
    font-family: var(--mono);
    font-size: 0.9em;
    color: var(--text);
    background: var(--panel-2);
    border: 1px solid var(--line);
    user-select: all;
    overflow-wrap: anywhere;
  }
</style>
