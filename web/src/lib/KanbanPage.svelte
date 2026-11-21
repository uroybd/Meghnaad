<script lang="ts">
  // The Kanban page (spec 0026): the list of kanbans, or one of them. Each column is the kanban's report run over its
  // filters (`columnArgs`), so which task is where, in what order and with which fields is the engine's answer; a task
  // is in the first column that has it, and Other holds what the report picks and no column does. A card dropped on a
  // column runs that column's On move on it.
  import { untrack } from 'svelte';
  import { runCli } from './api';
  import { toAddress } from './base';
  import { formatFor } from './dateformat';
  import { debounce } from './debounce';
  import FilterInput from './FilterInput.svelte';
  import { css, look } from './colors';
  import { formatTable, rowClass, type Cell } from './format';
  import type { Component } from 'svelte';
  import {
    Calendar,
    CalendarPlus,
    CalendarX,
    ChevronDown,
    ChevronRight,
    CircleCheck,
    CircleDot,
    Clock,
    FingerprintPattern,
    Flag,
    Folder,
    GitBranch,
    Hash,
    Hourglass,
    Link,
    Pencil,
    Play,
    Repeat,
    Settings,
    SlidersHorizontal,
    Tag,
    Zap,
  } from './icons';
  import { groupHeads } from './groups';
  import {
    cardShape,
    columnArgs,
    columnOf,
    fieldIcon,
    lanesOf,
    moveSteps,
    placeTasks,
    type Kanban,
    type KanbanView,
  } from './kanban';
  import ReportCell from './ReportCell.svelte';
  import { kanbanPath } from './route';
  import { scheme } from './scheme.svelte';
  import { store } from './store.svelte';
  import type { Column, ReportResult, Row } from './types';

  const kanban = $derived<Kanban | null>(store.kanbans.find((k) => k.id === store.kanbanId) ?? null);

  // What the page shows it with: the saved report, filter and Completed since, or what was chosen over them here.
  const report = $derived(store.kanbanReport ?? kanban?.report ?? 'next');
  let filter = $state('');
  let since = $state('');
  // A kanban opened (or its saved filter changed): what was typed over it this visit, else its own.
  $effect(() => {
    const k = kanban;
    if (!k) return;
    untrack(() => {
      filter = store.kanbanFilters[k.id] ?? k.filter;
      since = store.kanbanSince[k.id] ?? k.completedSince;
    });
  });
  // Typing waits until it stops before the columns are read again.
  let typed = $state<{ filter: string; since: string }>({ filter: '', since: '' });
  const settle = debounce((f: string, s: string) => (typed = { filter: f, since: s }), 400);
  $effect(() => {
    const k = kanban;
    if (!k) return;
    store.kanbanFilters[k.id] = filter;
    store.kanbanSince[k.id] = since;
    settle(filter, since);
  });
  $effect(() => {
    void kanban?.id;
    settle.flush();
  });

  interface Board {
    columns: Row[][];
    other: Row[];
    meta: Column[];
    /** Swimlanes from the report's groups (a `/` in its sort), none without them; and each task's lane. */
    lanes: string[];
    laneOf: Map<string, number>;
  }
  let board = $state<Board | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(false);
  let otherOpen = $state(false);
  let seq = 0;

  async function read(k: Kanban, v: KanbanView): Promise<Board> {
    const ask = async (args: string[]): Promise<ReportResult> => {
      const r = (await runCli({ args })).result;
      if (r.kind === 'error') throw new Error(r.message);
      if (r.kind !== 'report') throw new Error(`${v.report} is not a report.`);
      return r;
    };
    const results = [];
    // One at a time: the engine answers one command at a time anyway.
    for (const c of k.columns) results.push(await ask(columnArgs(v, c)));
    const all = await ask(columnArgs(v, null));
    const placed = placeTasks(
      results.map((r) => r.rows),
      all.rows,
    );
    return { ...placed, meta: all.columns, ...lanesOf(all.rows, groupHeads(all, ctx)) };
  }

  async function load() {
    const k = kanban;
    if (!k) return;
    const n = ++seq;
    loading = true;
    try {
      const b = await read(k, { report, filter: typed.filter, completedSince: typed.since });
      if (n === seq) {
        board = b;
        error = null;
      }
    } catch (e) {
      if (n === seq) error = e instanceof Error ? e.message : String(e);
    } finally {
      if (n === seq) loading = false;
    }
  }

  $effect(() => {
    void kanban;
    void report;
    void typed;
    void store.rev;
    void load();
  });

  // The cards' fields: the report's columns, as its table draws them.
  const settings = $derived(store.config?.config.settings);
  const ctx = $derived({
    now: store.now,
    udas: store.config?.config.udas ?? {},
    journal: store.config?.journal ?? null,
    dates: { report: formatFor('report', settings, undefined), annotation: formatFor('annotation', settings) },
    recurIndicator: settings?.['recurrence.indicator'] || undefined,
    indicators: {
      active: settings?.['active.indicator'],
      tag: settings?.['tag.indicator'],
      dependency: settings?.['dependency.indicator'],
    },
    transformations: store.appearance.transformations,
    idFallback: store.appearance.idFallback,
  });
  function cells(rows: Row[]): Cell[][] {
    return board && rows.length ? formatTable(board.meta, rows, ctx).cells : [];
  }
  // Appearance's `cardIcons`: each field by its icon, chosen by the field's name.
  const iconsOn = $derived(store.appearance.cardIcons);
  const ICON: Record<string, Component<{ size?: number }>> = {
    hash: Hash,
    'fingerprint-pattern': FingerprintPattern,
    'circle-dot': CircleDot,
    flag: Flag,
    folder: Folder,
    tag: Tag,
    clock: Clock,
    calendar: Calendar,
    hourglass: Hourglass,
    'calendar-x': CalendarX,
    'calendar-plus': CalendarPlus,
    pencil: Pencil,
    play: Play,
    'circle-check': CircleCheck,
    repeat: Repeat,
    link: Link,
    zap: Zap,
    'git-branch': GitBranch,
  };
  const iconOf = (name: string) => {
    const n = fieldIcon(name);
    return n ? ICON[n] : undefined;
  };
  /**
   * How a field shows. A pill only while Icons in cards is on: off, a UDA is its label then its value, as the rest. A
   * UDA drawn as a progress bar is the bar after its label, never a pill.
   */
  const shapeOf = (name: string, c: Cell | undefined) => {
    const shape = cardShape(name, ctx.udas, c?.text ?? '', !!c?.blocks);
    return shape === 'pill' && (!iconsOn || store.appearance.progress[name]) ? 'field' : shape;
  };
  /** The fields a card shows under its description: the report's other columns with something in them. */
  const fieldsOf = (row: Cell[]) =>
    (board?.meta ?? [])
      .map((col, i) => ({ col, c: row[i], shape: shapeOf(col.name, row[i]) }))
      .filter(({ col, c }) => col.name !== 'description' && c && c.text.trim() !== '');
  /** A card's edge takes the colour the table lays on the task's row (color.active, color.overdue, ...). */
  function edge(row: Row): string {
    const l = look(row.style, scheme.dark);
    const c = l?.background ?? l?.color;
    return c ? `--edge:${c}` : '';
  }
  /** A priority is drawn in its own colour, as the table draws it. */
  const priorityStyle = (name: string, row: Row) =>
    name === 'priority' && row.priority
      ? css(look(store.config?.colors?.[`uda.priority.${row.priority}`], scheme.dark))
      : undefined;
  const descriptionAt = $derived(board?.meta.findIndex((c) => c.name === 'description') ?? -1);

  // The columns take the height left in the page's area, and each scrolls its own cards, so every column is as tall as the
  // screen allows and a long one doesn't make the page scroll.
  let columnsEl = $state<HTMLElement>();
  let height = $state<number | null>(null);
  $effect(() => {
    const el = columnsEl;
    const main = el?.closest('main');
    if (!el || !main) return;
    const fit = () => {
      // The area's bottom padding (room for the + button), and a little for the scrollbar under the columns.
      const pad = parseFloat(getComputedStyle(main).paddingBottom) || 0;
      height = Math.max(
        320,
        main.getBoundingClientRect().bottom - (el.getBoundingClientRect().top + main.scrollTop) - pad - 16,
      );
    };
    fit();
    const ro = typeof ResizeObserver === 'function' ? new ResizeObserver(fit) : null;
    ro?.observe(main);
    if (el.parentElement) ro?.observe(el.parentElement);
    addEventListener('resize', fit);
    return () => {
      ro?.disconnect();
      removeEventListener('resize', fit);
    };
  });

  // Moving a card.
  // A card is dropped only in its own lane: a move runs the column's On move, which doesn't change the task's group.
  let dragging = $state<{ uuid: string; from: number; lane: number } | null>(null);
  let over = $state<{ col: number; lane: number } | null>(null);
  /** Whether the card being dragged can be dropped on column `i` of lane `lane` (-1: no lanes). */
  const droppable = (i: number, lane: number) =>
    !!dragging && dragging.from !== i && dragging.lane === lane && !!kanban?.columns[i]?.move.trim();
  const isOver = (i: number, lane: number) => over?.col === i && over.lane === lane && droppable(i, lane);
  /** A lane's cards in a column. */
  const inLane = (rows: Row[], lane: number) => rows.filter((r) => board?.laneOf.get(r.uuid) === lane);
  // Lanes folded on this page, by heading.
  let folded = $state<Set<string>>(new Set());
  function fold(lane: string) {
    const next = new Set(folded);
    if (!next.delete(lane)) next.add(lane);
    folded = next;
  }

  async function move(uuid: string, from: number, to: number) {
    const k = kanban;
    const col = k?.columns[to];
    if (!k || !col || !col.move.trim() || from === to) return;
    const ok = await store.actSteps(null, moveSteps(col.move, uuid, store.taskCommands));
    // `act` reads the page again through `rev`; read it now to say where the task went.
    await load();
    if (!ok || !board) return;
    const at = columnOf(board.columns, uuid);
    if (at !== to) {
      const where = at >= 0 ? k.columns[at].name : board.other.some((r) => r.uuid === uuid) ? 'Other' : null;
      store.notify(
        where
          ? `${col.move} ran, but the task is in ${where}, not ${col.name}: ${col.name}’s filter doesn’t match it.`
          : `${col.move} ran, and the task is no longer on this kanban.`,
        'err',
      );
    }
  }

  function chooseReport(name: string) {
    store.kanbanReport = kanban && name === kanban.report ? null : name;
  }

  // On a phone the report, filter and Completed since fold under a button, with a dot while they differ from the kanban's.
  let controlsOpen = $state(false);
  const changed = $derived(
    !!kanban &&
      (report !== kanban.report ||
        filter.trim() !== kanban.filter.trim() ||
        since.trim() !== kanban.completedSince.trim()),
  );

  function editKanbans() {
    store.openSettings('kanban');
  }
</script>

{#snippet card(row: Row, cs: Cell[], from: number)}
  <li
    class="card {rowClass(row)}"
    class:edged={!!edge(row)}
    style={edge(row)}
    data-pri={row.priority ?? undefined}
    draggable="true"
    ondragstart={(e) => {
      dragging = { uuid: row.uuid, from, lane: board?.lanes.length ? (board.laneOf.get(row.uuid) ?? 0) : -1 };
      e.dataTransfer?.setData('text/plain', row.uuid);
    }}
    ondragend={() => ((dragging = null), (over = null))}
  >
    <button class="open ghost" onclick={() => store.openDetail(row.uuid)}>
      {descriptionAt >= 0 && cs[descriptionAt] ? cs[descriptionAt].text : row.description}
    </button>
    <dl>
      {#each fieldsOf(cs).filter((f) => f.shape !== 'text') as f (f.col.spec)}
        {@const Icon = iconsOn ? iconOf(f.col.name) : undefined}
        <div class:badge={f.shape === 'pill'}>
          {#if Icon}<dt class="ic" title={f.col.label}>
              <Icon size={13} /><span class="sr-only">{f.col.label}</span>
            </dt>{:else}<dt>{f.col.label}</dt>{/if}
          <dd class="{f.col.kind} {f.c.cls ?? ''}" style={priorityStyle(f.col.name, row)}>
            <ReportCell col={f.col} c={f.c} {row} entry={null} activeTags={[]} />
          </dd>
        </div>
      {/each}
    </dl>
    <!-- A string UDA free to hold anything: a paragraph of its own, its checkboxes ticked here. -->
    {#each fieldsOf(cs).filter((f) => f.shape === 'text') as f (f.col.spec)}
      <div class="para">
        <h4>{f.col.label}</h4>
        <!-- The cell's classes (multiline: kept as written) on a box of its own, so the heading's line end isn't kept too. -->
        <div class={f.c.cls ?? ''}><ReportCell col={f.col} c={f.c} {row} entry={null} activeTags={[]} /></div>
      </div>
    {/each}
    {#if kanban && kanban.columns.some((c, i) => i !== from && c.move.trim())}
      <select
        class="moveto"
        aria-label="Move {row.description} to"
        value=""
        onchange={(e) => {
          const to = Number((e.currentTarget as HTMLSelectElement).value);
          (e.currentTarget as HTMLSelectElement).value = '';
          if (Number.isInteger(to)) void move(row.uuid, from, to);
        }}
      >
        <option value="" disabled>Move to…</option>
        {#each kanban.columns as c, i (i)}
          {#if i !== from && c.move.trim()}<option value={i}>{c.name}</option>{/if}
        {/each}
      </select>
    {/if}
  </li>
{/snippet}

{#snippet lanes(board: Board, kanban: Kanban)}
  <!-- Swimlanes: the column names once, then a row of the same columns for each of the report's groups. The board
       scrolls as a whole; a card moves only within its lane. -->
  <div
    class="board"
    class:loading
    aria-busy={loading}
    bind:this={columnsEl}
    style:height={height ? `${height}px` : undefined}
    style:--n={kanban.columns.length}
    style:--m={kanban.columns.length + 1}
  >
    <div class="grid" class:wide={otherOpen}>
      {#each kanban.columns as col, i (i)}
        <h2 class="colhead">
          {col.name} <span class="count dim">{board.columns[i]?.length ?? 0}</span>
        </h2>
      {/each}
      <h2 class="colhead">
        <button class="ghost fold" aria-expanded={otherOpen} onclick={() => (otherOpen = !otherOpen)}>
          {#if otherOpen}<ChevronDown size={14} />{:else}<ChevronRight size={14} />{/if} Other
        </button>
        <span class="count dim">{board.other.length}</span>
      </h2>
      {#each board.lanes as lane, l (lane)}
        {@const shut = folded.has(lane)}
        {@const count =
          board.columns.reduce((n, rows) => n + inLane(rows, l).length, 0) + inLane(board.other, l).length}
        <h3 class="lanehead">
          <button class="ghost fold" aria-expanded={!shut} onclick={() => fold(lane)}>
            {#if shut}<ChevronRight size={14} />{:else}<ChevronDown size={14} />{/if}
            {lane}
          </button>
          <span class="count dim">{count}</span>
        </h3>
        {#if !shut}
          {#each kanban.columns as col, i (i)}
            {@const rows = inLane(board.columns[i] ?? [], l)}
            {@const cs = cells(rows)}
            <section
              class="cell"
              class:over={isOver(i, l)}
              aria-label="{col.name}, {lane}"
              ondragover={(e) => {
                if (!droppable(i, l)) return;
                e.preventDefault();
                over = { col: i, lane: l };
              }}
              ondragleave={() => (over = isOver(i, l) ? null : over)}
              ondrop={(e) => {
                e.preventDefault();
                const ok = droppable(i, l);
                const d = dragging;
                dragging = null;
                over = null;
                if (d && ok) void move(d.uuid, d.from, i);
              }}
            >
              <ul class="cards">
                {#each rows as row, j (row.uuid)}{@render card(row, cs[j] ?? [], i)}{/each}
              </ul>
            </section>
          {/each}
          <section class="cell other" aria-label="Other, {lane}">
            {#if otherOpen}
              {@const rows = inLane(board.other, l)}
              {@const cs = cells(rows)}
              <ul class="cards">
                {#each rows as row, j (row.uuid)}{@render card(row, cs[j] ?? [], -1)}{/each}
              </ul>
            {/if}
          </section>
        {/if}
      {/each}
    </div>
  </div>
{/snippet}

<div class="page">
  {#if !store.kanbanSettings.enabled}
    <h1>Kanban</h1>
    <p class="dim">Kanban is off.</p>
    <button class="btn" onclick={editKanbans}><Settings size={14} /> Set it up in Settings → Kanban</button>
  {:else if !kanban}
    <h1>Kanbans</h1>
    {#if store.kanbans.length}
      <ul class="list">
        {#each store.kanbans as k (k.id)}
          <li>
            <a
              href={toAddress(kanbanPath(k.id))}
              onclick={(e) => {
                if (e.metaKey || e.ctrlKey || e.shiftKey || e.button) return;
                e.preventDefault();
                store.kanbanReport = null;
                store.kanbanId = k.id;
              }}><strong>{k.name}</strong></a
            >
            <span class="dim">on {k.report}, {k.columns.length} {k.columns.length === 1 ? 'column' : 'columns'}</span>
          </li>
        {/each}
      </ul>
    {:else}
      <p class="dim">No kanbans yet.</p>
    {/if}
    <button class="btn" onclick={editKanbans}><Settings size={14} /> New kanban or edit them in Settings</button>
  {:else}
    <header class="head">
      <h1>{kanban.name}</h1>
      <span class="grow"></span>
      <button
        class="ghost icon filters"
        aria-expanded={controlsOpen}
        aria-controls="kanban-controls"
        aria-label="Report, filter and Completed since"
        title="Report, filter and Completed since"
        onclick={() => (controlsOpen = !controlsOpen)}
        ><SlidersHorizontal size={17} />{#if changed}<span class="dot" title="Changed here"></span>{/if}</button
      >
      <button
        class="ghost icon"
        onclick={editKanbans}
        aria-label="Edit this kanban"
        title="Edit this kanban in Settings → Kanban"><Settings size={17} /></button
      >
    </header>
    <div class="controls" class:shut={!controlsOpen} id="kanban-controls">
      <label class="report"
        >Report
        <select value={report} onchange={(e) => chooseReport((e.currentTarget as HTMLSelectElement).value)}>
          {#if !store.offeredReports.some((r) => r.name === report)}<option value={report}>{report}</option>{/if}
          {#each store.offeredReports as r (r.name)}<option value={r.name}
              >{r.name}{r.name === kanban.report ? ' (default)' : ''}</option
            >{/each}
        </select>
      </label>
      <div class="filter">
        <FilterInput id="kanban-filter" bind:value={filter} placeholder="Filter: project:Home +work  (Tab completes)" />
      </div>
      <label class="since"
        >Completed since
        <input
          class="mono"
          bind:value={since}
          placeholder="today-14d"
          spellcheck="false"
          autocomplete="off"
          autocapitalize="off"
        />
      </label>
    </div>

    {#if error}<p class="err" role="alert">{error}</p>{/if}

    {#if !kanban.columns.length}
      <p class="dim">This kanban has no columns yet: add them under <strong>Edit</strong>.</p>
    {:else if board && board.lanes.length}
      {@render lanes(board, kanban)}
    {:else if board}
      <div
        class="columns"
        class:loading
        aria-busy={loading}
        bind:this={columnsEl}
        style:height={height ? `${height}px` : undefined}
      >
        {#each kanban.columns as col, i (i)}
          {@const rows = board.columns[i] ?? []}
          {@const cs = cells(rows)}
          {@const takes = !!col.move.trim()}
          <section
            class="column"
            class:over={isOver(i, -1)}
            aria-label={col.name}
            ondragover={(e) => {
              if (!droppable(i, -1)) return;
              e.preventDefault();
              over = { col: i, lane: -1 };
            }}
            ondragleave={() => (over = isOver(i, -1) ? null : over)}
            ondrop={(e) => {
              e.preventDefault();
              const d = dragging;
              dragging = null;
              over = null;
              if (d && takes) void move(d.uuid, d.from, i);
            }}
          >
            <h2>{col.name} <span class="count dim">{rows.length}</span></h2>
            {#if !takes}<p class="dim note">Takes no cards: it has no On move.</p>{/if}
            <ul class="cards">
              {#each rows as row, j (row.uuid)}{@render card(row, cs[j] ?? [], i)}{/each}
            </ul>
          </section>
        {/each}
        <section class="column other" class:open={otherOpen} aria-label="Other">
          <h2>
            <button class="ghost fold" aria-expanded={otherOpen} onclick={() => (otherOpen = !otherOpen)}>
              {#if otherOpen}<ChevronDown size={14} />{:else}<ChevronRight size={14} />{/if} Other
            </button>
            <span class="count dim">{board.other.length}</span>
          </h2>
          {#if otherOpen}
            {@const cs = cells(board.other)}
            <p class="dim note">What the report picks and no column does. Takes no cards.</p>
            <ul class="cards">
              {#each board.other as row, j (row.uuid)}{@render card(row, cs[j] ?? [], -1)}{/each}
            </ul>
          {/if}
        </section>
      </div>
    {:else if loading}
      <p class="dim">Loading…</p>
    {/if}
  {/if}
</div>

<style>
  .page {
    padding: 16px;
    max-width: 100%;
  }
  h1 {
    margin: 0 0 12px;
    font-size: 20px;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .grow {
    flex: 1;
  }
  .head .icon {
    position: relative;
    line-height: 0;
    padding: 6px;
  }
  .head .dot {
    position: absolute;
    top: 3px;
    right: 3px;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--accent);
  }
  /* The filter button is for a phone; a wider screen has the controls out. */
  .head .filters {
    display: none;
  }
  .btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  .list {
    list-style: none;
    padding: 0;
    margin: 0 0 16px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .list li {
    display: flex;
    gap: 10px;
    align-items: baseline;
    flex-wrap: wrap;
  }
  .controls {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    align-items: center;
    margin-bottom: 14px;
  }
  .controls label {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
  }
  .filter {
    flex: 1;
    min-width: 220px;
  }
  .since input {
    width: 120px;
  }
  .columns {
    display: flex;
    gap: 12px;
    align-items: stretch;
    overflow-x: auto;
    padding-bottom: 8px;
  }
  /* Swimlanes: one grid, the column names at the top and each lane's heading across it. */
  .board {
    overflow: auto;
    padding-bottom: 8px;
  }
  .board.loading {
    opacity: 0.7;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(var(--n), minmax(240px, 1fr)) 130px;
    max-width: calc(var(--n) * 440px + 130px + var(--n) * 12px);
    gap: 0 12px;
    align-items: start;
  }
  .grid.wide {
    grid-template-columns: repeat(var(--m), minmax(240px, 1fr));
    max-width: calc(var(--m) * 452px);
  }
  .colhead {
    height: 40px;
    white-space: nowrap;
    overflow: hidden;
    position: sticky;
    top: 0;
    z-index: 2;
    margin: 0;
    padding: 0 10px;
    font-size: 14px;
    display: flex;
    align-items: center;
    gap: 6px;
    background: var(--panel-2);
    border: 1px solid var(--line);
    border-radius: 8px 8px 0 0;
  }
  .lanehead {
    grid-column: 1 / -1;
    margin: 14px 0 6px;
    font-size: 13.5px;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .cell {
    background: var(--panel-2);
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: 8px;
    min-height: 56px;
    align-self: stretch;
  }
  .cell.other {
    background: transparent;
    border-style: dashed;
  }
  .cell.over {
    outline: 2px dashed var(--accent);
  }
  .columns.loading {
    opacity: 0.7;
  }
  /* Columns share the width, each between a card's least and most comfortable width; past that the row scrolls. */
  .column {
    flex: 1 1 300px;
    min-width: 240px;
    max-width: 440px;
    background: var(--panel-2);
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: 10px;
    min-height: 120px;
    display: flex;
    flex-direction: column;
  }
  .column.over {
    outline: 2px dashed var(--accent);
  }
  .column.other {
    flex: 0 0 130px;
    min-width: 0;
    min-height: 0;
    align-self: flex-start;
  }
  .column.other.open {
    align-self: stretch;
  }
  .column.other.open {
    flex: 1 1 300px;
    min-width: 240px;
  }
  .column h2 {
    margin: 0 0 8px;
    font-size: 14px;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .fold {
    white-space: nowrap;
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 0;
    font: inherit;
    font-weight: 600;
  }
  .note {
    font-size: 12px;
    margin: 0 0 8px;
  }
  .cards {
    list-style: none;
    margin: 0;
    padding: 0 2px 2px 0;
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  /* Spaced as the phone's task cards are (TaskCard.kt): room inside, a line between the title and the fields. */
  .card {
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: 11px 12px 12px 14px;
    cursor: grab;
    display: flex;
    flex-direction: column;
    gap: 9px;
  }
  .open {
    text-align: left;
    font-weight: 600;
    font-size: 14.5px;
    line-height: 1.35;
    padding: 0;
    overflow-wrap: anywhere;
  }
  /* The fields side by side, as many as fit on a line. */
  dl {
    margin: 0;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 7px 14px;
    font-size: 12.5px;
  }
  dl div {
    display: flex;
    gap: 5px;
    min-width: 0;
    max-width: 100%;
  }
  dt {
    color: var(--dim);
    flex: 0 0 auto;
  }
  dd {
    margin: 0;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  dt.ic {
    display: inline-flex;
    align-items: center;
    opacity: 0.85;
  }
  /* A free-text UDA: a paragraph under the fields, as written. */
  .para {
    font-size: 13px;
    overflow-wrap: anywhere;
  }
  .para h4 {
    margin: 2px 0 2px;
    font-size: 14px;
    font-weight: 700;
  }
  .para .multiline {
    white-space: pre-wrap;
  }
  .para :global(.md) :global(:is(p, ul, ol)) {
    margin: 0;
  }
  /* A UDA: a pill, its label on one half and its value on the other. */
  dl div.badge {
    gap: 0;
    border: 1px solid var(--line);
    border-radius: 4px;
    overflow: hidden;
    line-height: 1.5;
  }
  .badge dt {
    padding: 0 6px;
    background: var(--panel-2);
  }
  .badge dd {
    padding: 0 6px;
    background: color-mix(in srgb, var(--accent) 14%, transparent);
  }
  /* As the table draws them: a priority in its colour, a finished task tinted, a waiting one dim. */
  dd.priority {
    font-weight: 600;
  }
  .card[data-pri='H'] dd.priority {
    color: var(--pri-h);
  }
  .card[data-pri='M'] dd.priority {
    color: var(--pri-m);
  }
  .card[data-pri='L'] dd.priority {
    color: var(--pri-l);
  }
  dd.dim,
  .card.done,
  .card.waiting {
    color: var(--dim);
  }
  .card.completed {
    background: color-mix(in srgb, var(--ok) 9%, var(--panel));
  }
  .card.deleted {
    background: color-mix(in srgb, var(--err) 9%, var(--panel));
  }
  .card.edged {
    border-color: var(--edge);
    box-shadow: inset 3px 0 0 var(--edge);
  }
  .moveto {
    font-size: 12px;
    align-self: flex-start;
  }
  /* Dragging is the way with a mouse; Move to… is for a phone or a touch screen, where dragging is hard. */
  @media (hover: hover) and (pointer: fine) and (min-width: 761px) {
    .moveto {
      display: none;
    }
  }
  @media (max-width: 760px) {
    .head .filters {
      display: inline-block;
    }
    .controls.shut {
      display: none;
    }
    .grid,
    .grid.wide {
      grid-template-columns: repeat(var(--n), 82vw) 130px;
      max-width: none;
    }
    .grid.wide {
      grid-template-columns: repeat(var(--m), 82vw);
    }
    .lanehead {
      position: sticky;
      left: 0;
      width: calc(100vw - 52px);
    }
    .column,
    .column.other.open {
      flex: 0 0 82vw;
      max-width: none;
    }
  }
</style>
