<script lang="ts">
  import { css, look, rowVars } from './colors';
  import { rowClass, type Cell } from './format';
  import ReportCell from './ReportCell.svelte';
  import RowActions from './RowActions.svelte';
  import { scheme } from './scheme.svelte';
  import { selection } from './selection.svelte';
  import { store, type Entry } from './store.svelte';
  import type { Column, ReportResult, Row } from './types';

  let {
    result,
    rows,
    shown,
    cells,
    heads,
    entry,
    selectable,
    activeTags,
    ontag,
    onedit,
    folded,
  }: {
    result: ReportResult;
    /** The rows drawn so far: the first of the report's (see `lazyRows`). */
    rows: Row[];
    /** The columns shown, each with its place among the report's columns. */
    shown: { col: Column; ci: number }[];
    /** What each cell shows, by row and by the report's column. */
    cells: Cell[][];
    /** A header above the row of that index, where a group starts. */
    heads: (string | null)[];
    entry: Entry | null;
    selectable: boolean;
    activeTags: string[];
    ontag?: (tag: string) => void;
    onedit?: (row: Row) => void;
    /** The table is too wide for a row's buttons: they are in its ⋯ menu. */
    folded: boolean;
  } = $props();

  let pending = $state<string | null>(null);

  // On a phone the rows are cards (the same width as the CSS below), and a card keeps one button, Done, with the rest in its ⋯
  // menu: it is compact, and a thumb has fewer to miss.
  let card = $state(false);
  $effect(() => {
    if (typeof matchMedia !== 'function') return;
    const q = matchMedia('(max-width: 760px)');
    card = q.matches;
    const on = (e: MediaQueryListEvent) => (card = e.matches);
    q.addEventListener('change', on);
    return () => q.removeEventListener('change', on);
  });

  // A click anywhere on the row opens the detail view, except on controls and selected text.
  function open(e: MouseEvent | KeyboardEvent, row: Row) {
    const t = e.target as HTMLElement;
    if (t.closest('button, a, input, select, textarea')) return;
    if (e instanceof MouseEvent && window.getSelection()?.toString()) return;
    store.openDetail(row.uuid, entry);
  }

  const isOpen = (row: Row) => row.status === 'pending';
  /** A card's ribbon, on a phone: the priority in the colour the taskrc gives it (color.uda.priority.H), if any. */
  function ribbon(row: Row): string {
    const p = row.priority ? look(store.config?.colors?.[`uda.priority.${row.priority}`], scheme.dark) : null;
    const c = p?.background ?? p?.color;
    return c ? `--ribbon:${c}` : '';
  }
</script>

<tbody>
  {#each rows as row, i (row.uuid)}
    {#if heads[i]}
      <tr class="grouphead"><th colspan={shown.length + 1 + (selectable ? 1 : 0)} scope="colgroup">{heads[i]}</th></tr>
    {/if}
    {@const l = look(row.style, scheme.dark)}
    <tr
      class={rowClass(row)}
      class:ruled={!!l}
      style={[rowVars(l), ribbon(row)].filter(Boolean).join(';')}
      class:gap={result.breaks[i] && heads.length === 0}
      class:selected={store.detail?.uuid === row.uuid}
      class:picked={selectable && selection.has(row.uuid)}
      class:selectable
      class:overdue={isOpen(row) && row.due != null && row.due < store.now}
      class:started={row.start != null && row.end == null}
      data-pri={row.priority ?? undefined}
      data-uuid={row.uuid}
      tabindex="0"
      aria-label="Open details: {row.description}"
      onclick={(e) => open(e, row)}
      onkeydown={(e) => e.key === 'Enter' && e.target === e.currentTarget && open(e, row)}
    >
      {#if selectable}
        <td class="sel">
          <label class="selbox">
            <input
              type="checkbox"
              checked={selection.has(row.uuid)}
              aria-label="Select: {row.description}"
              onchange={() => selection.toggle(row.uuid)}
            />
          </label>
        </td>
      {/if}
      {#each shown as { col, ci } (col.spec)}
        {@const c = cells[i][ci]}
        <td
          class="{col.kind} {c.cls ?? ''} col-{col.name.replace(/\W+/g, '-')}{col.format ? ` fmt-${col.format}` : ''}"
          class:blank={!c.text && !c.notes?.length}
          data-label={col.label}
          style={col.name === 'priority' && row.priority
            ? css(look(store.config?.colors?.[`uda.priority.${row.priority}`], scheme.dark))
            : undefined}
        >
          <ReportCell {col} {c} {row} {entry} {activeTags} {ontag} />
        </td>
      {/each}
      <td class="actions">
        <RowActions {row} {entry} {onedit} {card} {folded} bind:pending />
      </td>
    </tr>
  {/each}
</tbody>

<style>
  th,
  td {
    text-align: left;
    padding: 9px 18px 9px 0;
    vertical-align: top;
    white-space: nowrap;
  }
  tbody td {
    border-bottom: 1px solid color-mix(in srgb, var(--line) 60%, transparent);
  }
  /* The header above a group of rows is a `th`: drawn as the table's headers are, then as its own. */
  th {
    color: var(--dim);
    font-weight: 500;
    font-size: 13px;
    padding-bottom: 10px;
    border-bottom: 1px solid var(--line);
  }
  tr.grouphead,
  tr.grouphead:hover {
    cursor: default;
    background: transparent;
  }
  tr.grouphead th {
    padding: 14px 0 4px;
    border-bottom: 1px solid var(--accent);
    color: var(--accent);
    font-size: 13px;
    font-weight: 700;
    text-align: left;
    white-space: normal;
  }
  td.description {
    white-space: normal;
    min-width: 16em;
  }
  /* A text column never squeezes below a few words, however wide the description wants to be. */
  td.string {
    min-width: 6em;
  }
  td.id,
  td.number {
    font-variant-numeric: tabular-nums;
  }
  td.priority {
    font-weight: 600;
  }
  td.pri-h,
  .pri-h {
    color: var(--pri-h);
  }
  td.pri-m,
  .pri-m {
    color: var(--pri-m);
  }
  td.pri-l,
  .pri-l {
    color: var(--pri-l);
  }
  td.id:not(.dim) {
    font-weight: 700;
  }
  td.dim {
    color: var(--dim);
  }
  tbody tr {
    cursor: pointer;
  }
  tr.gap td {
    border-top: 14px solid transparent;
  }
  /* Finished tasks are shown by colour, not struck through: green for completed, red for deleted. */
  tr.done td {
    color: var(--dim);
  }
  tr.completed td {
    background: color-mix(in srgb, var(--ok) 9%, transparent);
  }
  tr.deleted td {
    background: color-mix(in srgb, var(--err) 9%, transparent);
  }
  tr.waiting td {
    color: var(--dim);
  }
  /* The colour rules (color.active, color.overdue, ...) lay their colour on the whole row. */
  tr.ruled td {
    color: var(--row-fg, inherit);
    background: var(--row-bg, transparent);
    font-weight: var(--row-weight, inherit);
    text-decoration: var(--row-deco, none);
  }
  tbody tr:hover {
    background: var(--panel-2);
  }
  tr.selected {
    background: var(--panel-2);
    box-shadow: inset 3px 0 0 var(--accent);
  }
  /* The ticked rows: a tint, so a selection reads at a glance across the table. */
  tr.picked,
  tr.picked:hover {
    background: color-mix(in srgb, var(--accent) 10%, transparent);
  }
  /*
   * The pinned Actions column (the table marks itself `.pinned`) covers what scrolls under it, so it paints the row's own
   * colour over the page's: each state of a row says what tint it has (--act-tint), in the order the row's own backgrounds win.
   */
  @media (min-width: 761px) {
    :global(.pinned) td.actions {
      position: sticky;
      right: 0;
    }
    /* The cell of an open menu is above the cells of the rows after it, which are pinned over the page the same way. */
    :global(.pinned) tbody tr td.actions:has(:global(.menu)) {
      z-index: 3;
    }
    :global(.pinned) tbody tr td.actions {
      z-index: 1;
      padding-left: 10px;
      background-color: var(--bg);
      background-image: linear-gradient(var(--act-tint, transparent), var(--act-tint, transparent));
      box-shadow: -8px 0 8px -8px rgb(0 0 0 / 0.2);
    }
    /* Every rule has the same weight, then the :hover form, so the later one wins: the order the row's cells win in. */
    :global(.pinned) tbody tr:hover,
    :global(.pinned) tbody tr.selected {
      --act-tint: var(--panel-2);
    }
    :global(.pinned) tbody tr.picked,
    :global(.pinned) tbody tr.picked:hover {
      --act-tint: color-mix(in srgb, var(--accent) 10%, transparent);
    }
    :global(.pinned) tbody tr.completed,
    :global(.pinned) tbody tr.completed:hover {
      --act-tint: color-mix(in srgb, var(--ok) 9%, transparent);
    }
    :global(.pinned) tbody tr.deleted,
    :global(.pinned) tbody tr.deleted:hover {
      --act-tint: color-mix(in srgb, var(--err) 9%, transparent);
    }
    :global(.pinned) tbody tr.ruled,
    :global(.pinned) tbody tr.ruled:hover {
      --act-tint: var(--row-bg, transparent);
    }
  }
  td.sel {
    width: 1%;
    padding-right: 6px;
  }
  /* The whole cell is the target, not just the box. */
  .selbox {
    display: flex;
    align-items: center;
    justify-content: center;
    min-width: 28px;
    min-height: 24px;
    cursor: pointer;
  }
  td.sel input {
    cursor: pointer;
  }
  /* Room for the selection bar (and the hover tint) so the first column never touches it. */
  @media (min-width: 761px) {
    th:first-child,
    td:first-child {
      padding-left: 12px;
    }
  }
  tr:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  td.multiline {
    white-space: pre-line;
  }
  .actions {
    text-align: right;
    white-space: nowrap;
  }

  /*
   * Phones: each task is a card. The description comes first, with the tick box beside it and the task's number in the
   * corner; what is known of the task follows as a row of small marks (a few with an icon in place of their name); the
   * buttons share that row when there is room, and take their own when there is not. A ribbon down the left edge is the
   * priority, in the colour the taskrc gives it, as on every front end, and a started task is tinted.
   */
  @media (max-width: 760px) {
    tbody {
      display: block;
    }
    tbody tr {
      /* The little pictures, as masks, so they take the colour of the text. */
      --i-folder: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z'/%3E%3C/svg%3E");
      --i-clock: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Ccircle cx='12' cy='12' r='10'/%3E%3Cpolyline points='12 6 12 12 16 14'/%3E%3C/svg%3E");
      --i-flag: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='M4 15s1-1 4-1 5 2 8 2 4-1 4-1V3s-1 1-4 1-5-2-8-2-4 1-4 1z'/%3E%3Cline x1='4' x2='4' y1='22' y2='15'/%3E%3C/svg%3E");
      --i-zap: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='M4 14a1 1 0 0 1-.78-1.63l9.9-10.2a.5.5 0 0 1 .86.46l-1.92 6.02A1 1 0 0 0 13 10h7a1 1 0 0 1 .78 1.63l-9.9 10.2a.5.5 0 0 1-.86-.46l1.92-6.02A1 1 0 0 0 11 14z'/%3E%3C/svg%3E");
      --i-repeat: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='m17 2 4 4-4 4'/%3E%3Cpath d='M3 11v-1a4 4 0 0 1 4-4h14'/%3E%3Cpath d='m7 22-4-4 4-4'/%3E%3Cpath d='M21 13v1a4 4 0 0 1-4 4H3'/%3E%3C/svg%3E");
      --i-link: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71'/%3E%3Cpath d='M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71'/%3E%3C/svg%3E");
      --i-tag: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='M12.586 2.586A2 2 0 0 0 11.172 2H4a2 2 0 0 0-2 2v7.172a2 2 0 0 0 .586 1.414l8.704 8.704a2.426 2.426 0 0 0 3.42 0l6.58-6.58a2.426 2.426 0 0 0 0-3.42z'/%3E%3C/svg%3E");
      --i-play: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpolygon points='6 3 20 12 6 21 6 3'/%3E%3C/svg%3E");
      --i-calendar: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Crect width='18' height='18' x='3' y='4' rx='2'/%3E%3Cpath d='M16 2v4M8 2v4M3 10h18'/%3E%3C/svg%3E");
      position: relative;
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: 4px 10px;
      padding: 9px 8px 7px 15px;
      margin-bottom: 8px;
      border: 1px solid var(--line);
      border-radius: 14px;
      background: var(--panel);
      /* The priority, as a ribbon down the left edge: the taskrc's colour for it, else the page's own. */
      box-shadow: inset 4px 0 0 var(--ribbon, var(--stripe, transparent));
    }
    tbody tr[data-pri='H'] {
      --stripe: var(--pri-h);
    }
    tbody tr[data-pri='M'] {
      --stripe: var(--pri-m);
    }
    tbody tr[data-pri='L'] {
      --stripe: var(--pri-l);
    }
    tbody tr.started {
      background: color-mix(in srgb, var(--accent) 7%, var(--panel));
    }
    tr.gap {
      margin-top: 18px;
    }
    tr.grouphead {
      display: block;
      margin: 14px 0 6px;
      padding: 0;
      border: 0;
      border-radius: 0;
      box-shadow: none;
      background: transparent;
    }
    tr.grouphead th {
      display: block;
      padding: 0 2px 4px;
    }
    tr.gap td {
      border-top: 0;
    }
    /* No rule under each mark: the table's, which would underline every piece of the card. */
    tbody td {
      display: inline-flex;
      align-items: center;
      flex-wrap: wrap;
      gap: 5px;
      padding: 0;
      border: 0;
      font-size: 13px;
      line-height: 1.3;
      white-space: normal;
      color: var(--dim);
    }
    /* A mark's name, small, for the columns that have no picture. */
    td::before {
      content: attr(data-label);
      font-size: 11px;
      text-transform: uppercase;
      letter-spacing: 0.05em;
      opacity: 0.75;
    }
    td.blank {
      display: none;
    }
    /* The marks that matter first, in this order; the rest (age, recurrence, ...) after them. */
    td:not(.description, .sel, .id, .actions) {
      order: 6;
    }
    td.col-priority {
      order: 1;
    }
    td.col-project {
      order: 2;
    }
    td.tags {
      order: 3;
    }
    td.col-due {
      order: 4;
    }
    td.col-urgency {
      order: 5;
    }
    td.col-project::before,
    td.col-due::before,
    td.col-priority::before,
    td.col-urgency::before,
    td.col-scheduled::before {
      content: '';
      flex: none;
      width: 13px;
      height: 13px;
      opacity: 0.85;
      background: currentColor;
      -webkit-mask: var(--mask) center / contain no-repeat;
      mask: var(--mask) center / contain no-repeat;
    }
    td.col-project::before {
      --mask: var(--i-folder);
    }
    td.col-due::before {
      --mask: var(--i-clock);
    }
    td.col-priority::before {
      --mask: var(--i-flag);
    }
    td.col-urgency::before {
      --mask: var(--i-zap);
    }
    td.col-scheduled::before {
      --mask: var(--i-calendar);
    }
    td.col-tags::before {
      display: none;
    }
    /*
     * A column that shows only an indicator letter (the recurrence's R, the dependency's D, the tag's +, the active *): its
     * name and its letter would read "R R", so it is a small picture of what it means instead.
     */
    td.fmt-indicator,
    td.fmt-active {
      font-size: 0;
      gap: 0;
    }
    td.fmt-indicator::before,
    td.fmt-active::before {
      display: inline-block;
      content: '';
      width: 14px;
      height: 14px;
      background: currentColor;
      opacity: 0.8;
      -webkit-mask: var(--mask) center / contain no-repeat;
      mask: var(--mask) center / contain no-repeat;
    }
    td.col-recur::before {
      --mask: var(--i-repeat);
    }
    td.col-depends::before {
      --mask: var(--i-link);
    }
    td.col-tags.fmt-indicator::before {
      --mask: var(--i-tag);
    }
    td.col-start.fmt-active::before {
      --mask: var(--i-play);
    }
    /* A due date that has passed says so; so does a task due today. */
    tr.overdue td.col-due {
      color: var(--err);
      font-weight: 600;
    }
    /* The priority as a small pill of its own colour. */
    td.col-priority {
      padding: 1px 8px 1px 6px;
      border-radius: 999px;
      font-weight: 700;
      background: color-mix(in srgb, currentColor 13%, transparent);
    }
    td.col-urgency {
      font-variant-numeric: tabular-nums;
    }
    td.description {
      order: -1;
      flex-basis: 100%;
      font-size: 15.5px;
      font-weight: 500;
      line-height: 1.35;
      color: var(--text);
      padding-right: 3.2em;
      min-width: 0;
    }
    /* The tick box sits in the card's top left corner, with the description beside it. */
    td.sel {
      position: absolute;
      top: 6px;
      left: 7px;
      width: auto;
      padding: 0;
    }
    td.sel::before {
      display: none;
    }
    tr.selectable td.description {
      padding-left: 33px;
    }
    .selbox {
      min-width: 36px;
      min-height: 36px;
    }
    td.description::before,
    td.actions::before {
      display: none;
    }
    /* The task's number, a small badge in the corner. */
    td.id {
      position: absolute;
      top: 11px;
      right: 11px;
      padding: 0 7px;
      border-radius: 999px;
      font-size: 11.5px;
      background: var(--panel-2);
    }
    td.id::before {
      content: '#';
      font-size: inherit;
      text-transform: none;
      letter-spacing: 0;
      opacity: 0.6;
    }
    /* The tags: small pills, close together (the button's own height for a thumb is for the buttons below). */
    td.tags {
      gap: 4px;
    }
    td.tags :global(.tagpill) {
      min-height: 0;
      height: 21px;
      margin: 0;
      padding: 0 9px;
      font-size: 12px;
      line-height: 19px;
    }
    tr.done td.description,
    tr.done td {
      color: var(--dim);
    }
    /* A card is one box: tint the box, not each piece of it. */
    tr.completed,
    tr.deleted {
      background: color-mix(in srgb, var(--ok) 9%, var(--panel));
    }
    tr.deleted {
      background: color-mix(in srgb, var(--err) 9%, var(--panel));
    }
    tr.completed td,
    tr.deleted td,
    tr.ruled td {
      background: transparent;
    }
    /* A card is one box: the rule's background is the card's. */
    tr.ruled {
      background: var(--row-bg, var(--panel));
    }
    /* The buttons (`RowActions`): on the same line as the marks when there is room. */
    td.actions {
      order: 99;
      display: flex;
      flex: none;
      margin-left: auto;
      gap: 2px;
    }
    /* A field written in Markdown is a few lines: on a card it sits in a faint frame of its own, label and all, so it reads
       as a field beneath the task and not as more of its line. Its text and the notes keep the plain weight a started or
       coloured task's description does not pass on to them (the Android app's cards do the same). */
    td:has(> :global(.md)) {
      display: block;
      flex-basis: 100%;
      padding: 6px 10px 8px;
      border: 1px solid color-mix(in srgb, var(--line) 70%, transparent);
      border-radius: 6px;
    }
  }
</style>
