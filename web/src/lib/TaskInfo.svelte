<script lang="ts">
  import { runCli } from './api';
  import { formatFor } from './dateformat';
  import { formatMoment, formatSeconds, formatStamp } from './dates';
  import { projectSegments, shortUuid, trimNumber, udaLabel, urgencyLevel } from './format';
  import { CornerUpLeft, Lock, Pencil, Repeat, Timer } from './icons';
  import { timeLog, visibleNotes } from './journal';
  import Description from './Description.svelte';
  import MarkdownNodes from './MarkdownNodes.svelte';
  import { trees } from './markdown';
  import ProjectPath from './ProjectPath.svelte';
  import TagChip from './TagChip.svelte';
  import { describeRecur } from './recurrence';
  import StatusPill, { type Kind } from './StatusPill.svelte';
  import DepItem from './DepItem.svelte';
  import DepLink from './DepLink.svelte';
  import UuidTip from './UuidTip.svelte';
  import { store } from './store.svelte';
  import TaskHistory from './TaskHistory.svelte';
  import TaskSessions from './TaskSessions.svelte';
  import type { Row } from './types';
  import { urgencyBreakdown } from './urgencyBreakdown';

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

  // How the urgency adds up, as `task info` shows it: asked of the engine when it is opened.
  let urgencyOpen = $state(false);
  const breakdown = $derived.by(() => {
    if (!urgencyOpen) return null;
    try {
      return urgencyBreakdown(task, store.config?.config);
    } catch {
      return null;
    }
  });

  const defs = $derived(store.config?.config.udas ?? {});
  const infoFmt = $derived(formatFor('info', store.config?.config.settings));
  const noteFmt = $derived(formatFor('infoNote', store.config?.config.settings));
  // History moments carry seconds (a row is the changes within one second); a configured
  // `dateformat.info` (else its default) is used.
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
  // A string UDA and a note may hold Markdown: its tree, or null for text that is only words.
  const udaTrees = $derived(
    Object.fromEntries(
      (() => {
        const names = udaKeys.filter((k) => defs[k].type === 'string');
        return trees(
          names.map((k) => task.extra[k]),
          names,
          store.rules,
          names.map(() => task.uuid),
        ).map((t, i) => [names[i], t]);
      })(),
    ),
  );
  const noteTrees = $derived(
    trees(
      notes.map((a) => a.text),
      notes.map(() => 'annotations'),
      store.rules,
      notes.map(() => task.uuid),
    ),
  );
  // The sessions, from the journal's notes by the engine's own helper (as the phone and the terminal app read them).
  const log = $derived(timeLog(task, store.config?.journal));
  const totalTracked = $derived(task.active_seconds ?? log.total);
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
      const { result } = await runCli({
        args: ['(', 'status:pending', 'or', 'status:waiting', ')', `depends.has:${task.uuid}`, 'all'],
      });
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
      <strong class="grow"><Description text={task.description} uuid={task.uuid} /></strong>
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
      {#if udaTrees[k]}
        <dd class="md">
          <MarkdownNodes
            nodes={udaTrees[k]}
            ontoggle={(line) => store.tickBox(null, task.uuid, k, task.extra[k], line)}
          />
        </dd>
      {:else}
        <dd class="pre">{defs[k].type === 'numeric' ? trimNumber(task.extra[k]) : task.extra[k]}</dd>
      {/if}
    {/each}
    <dt>Urgency</dt>
    <dd>
      <button
        class="urg urg-{urgencyLevel(task.urgency)}"
        aria-expanded={urgencyOpen}
        title={urgencyOpen ? 'Hide how it adds up' : 'Show how it adds up'}
        onclick={() => (urgencyOpen = !urgencyOpen)}>{task.urgency.toFixed(2)}</button
      >
      {#if breakdown}
        <table class="urgency mono" aria-label="How the urgency adds up">
          <tbody>
            {#each breakdown.terms as [label, measure, coefficient, value] (label)}
              <tr>
                <th scope="row">{label}</th>
                <td>{measure}</td>
                <td class="dim">×</td>
                <td>{coefficient}</td>
                <td class="dim">=</td>
                <td>{value}</td>
              </tr>
            {/each}
            <tr class="total">
              <th scope="row">urgency</th>
              <td colspan="4"></td>
              <td>{breakdown.total}</td>
            </tr>
          </tbody>
        </table>
      {/if}
    </dd>
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

  {#if log.sessions.length}
    <TaskSessions sessions={log.sessions} total={totalTracked} {stamp} />
  {/if}

  {#if notes.length}
    <h4>Annotations</h4>
    <ul>
      {#each notes as a, i (a.entry)}<li>
          <span class="dim">{formatMoment(a.entry, undefined, noteFmt)}</span>
          {#if noteTrees[i]}
            <div class="md">
              <MarkdownNodes nodes={noteTrees[i]} />
            </div>
          {:else}
            <span class="pre">{a.text}</span>
          {/if}
        </li>{/each}
    </ul>
  {/if}

  <TaskHistory {task} {stamp} />

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
  .urg {
    font: inherit;
    padding: 0;
    border: 0;
    background: none;
    cursor: pointer;
    text-decoration: underline dotted;
    text-underline-offset: 3px;
  }
  .urgency {
    margin-top: 6px;
    border-collapse: collapse;
    font-size: 12px;
  }
  .urgency th {
    font-weight: 400;
    text-align: left;
    padding-right: 12px;
  }
  .urgency td {
    text-align: right;
    padding: 0 4px;
  }
  .urgency .total {
    border-top: 1px solid var(--border);
  }
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
  .md {
    white-space: normal;
    min-width: 0;
  }
  li > .md {
    margin-top: 2px;
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
