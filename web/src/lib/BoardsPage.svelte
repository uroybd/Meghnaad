<script lang="ts">
  import BoardView from './BoardView.svelte';
  import FilterInput from './FilterInput.svelte';
  import type { Homepage } from './appsettings';
  import { ChevronLeft, LayoutDashboard, Pencil, Plus, Trash2, TriangleAlert } from './icons';
  import SectionForm from './SectionForm.svelte';
  import {
    SECTION_TYPES,
    newBoard,
    newSection,
    normalizeBoards,
    slugProblem,
    type Board,
    type Section,
    type SectionType,
  } from './boards';
  import { boardPath } from './route';
  import { store } from './store.svelte';

  const board = $derived(store.boards.find((b) => b.id === store.boardId) ?? null);

  /** The board being edited: a copy, so Cancel leaves the saved one alone. */
  let draft = $state<Board | null>(null);
  /** The slug the draft had when it was opened, which it may change. */
  let from = $state<string | null>(null);
  let selected = $state<string | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let newName = $state('');
  let confirmDelete = $state(false);

  // A board that is gone (deleted from another browser, or by a hand edit of the file) is not open any more.
  $effect(() => {
    // (Not while saving: a board whose slug is changing is not at its old address any more and not yet at its new one.)
    if (!busy && store.boardId && store.appSettings && !board) {
      store.boardId = null;
      draft = null;
    }
  });

  // What the engine says of the draft: the sections it would drop, and the board it would keep.
  const check = $derived(draft ? normalizeBoards([$state.snapshot(draft)]) : null);
  const problems = $derived(check?.errors ?? []);
  const slugError = $derived(
    draft
      ? slugProblem(
          draft.id,
          store.boards.filter((b) => b.id !== from).map((b) => b.id),
        )
      : null,
  );
  const valid = $derived(!!check && check.errors.length === 0 && check.boards.length === 1 && !slugError);
  const picked = $derived(draft?.sections.find((s) => s.id === selected) ?? null);

  function open(id: string) {
    store.boardId = id;
    draft = null;
    selected = null;
    error = null;
  }

  function edit() {
    if (!board) return;
    draft = $state.snapshot(board) as Board;
    from = board.id;
    selected = null;
    error = null;
  }

  function cancel() {
    draft = null;
    from = null;
    selected = null;
    error = null;
  }

  /**
   * Save the boards, and the home page too when it has to move with a board; then choose `select` as the board shown, before
   * anything can notice the board it had is gone.
   */
  async function persist(next: Board[], homepage?: Homepage | null, select?: string) {
    busy = true;
    error = null;
    try {
      await store.saveBoards(next, homepage);
      if (select) store.boardId = select;
      return true;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
      return false;
    } finally {
      busy = false;
    }
  }

  async function save() {
    // The tidied board leaves out what the engine refuses, so a draft with a problem is not saved at all.
    if (!draft || !valid || !check?.boards[0]) return;
    const kept = check.boards[0];
    const was = from ?? kept.id;
    // The home page names a board by its slug: if that is the board whose slug changed, it follows.
    const home =
      store.homepage && 'board' in store.homepage && store.homepage.board === was && kept.id !== was
        ? { board: kept.id }
        : undefined;
    if (
      await persist(
        store.boards.map((b) => (b.id === was ? kept : b)),
        home,
        kept.id,
      )
    ) {
      draft = null;
      from = null;
      selected = null;
    }
  }

  async function create(e: SubmitEvent) {
    e.preventDefault();
    const name = newName.trim();
    if (!name) return;
    const made = newBoard(name, store.boards);
    if (await persist([...store.boards, made], undefined, made.id)) {
      newName = '';
      draft = $state.snapshot(made) as Board;
      from = made.id;
    }
  }

  async function remove() {
    if (!board) return;
    if (!confirmDelete) {
      confirmDelete = true;
      setTimeout(() => (confirmDelete = false), 4000);
      return;
    }
    confirmDelete = false;
    const id = board.id;
    if (await persist(store.boards.filter((b) => b.id !== id))) {
      store.boardId = null;
      draft = null;
    }
  }

  function add(type: SectionType) {
    if (!draft) return;
    const s = newSection(draft, type);
    draft.sections = [...draft.sections, s];
    selected = s.id;
  }

  const setSections = (sections: Section[]) => draft && (draft.sections = sections);
  const setSection = (next: Section) =>
    draft && (draft.sections = draft.sections.map((s) => (s.id === next.id ? next : s)));
  function removeSection(id: string) {
    if (!draft) return;
    draft.sections = draft.sections.filter((s) => s.id !== id);
    if (selected === id) selected = null;
  }
</script>

<div class="page">
  {#if store.appSettings?.boardErrors.length}
    <div class="warn" role="alert">
      <TriangleAlert size={15} />
      <div>
        Some of what is saved for boards was not understood and is left out until it is fixed in <code
          >settings.json</code
        >:
        <ul>
          {#each store.appSettings.boardErrors as line, i (i)}<li>{line}</li>{/each}
        </ul>
      </div>
    </div>
  {/if}

  {#if !board}
    <h1>Boards</h1>
    <p class="dim">
      A board is a page of numbers, tables and charts, each with its own filter. Boards are kept in this app's
      <code>settings.json</code>, not in your taskrc.
    </p>
    <form class="new row" onsubmit={create}>
      <input bind:value={newName} placeholder="Name of a new board" aria-label="Name of a new board" />
      <button class="primary" disabled={busy || !newName.trim()}><Plus size={15} /> New board</button>
    </form>
    {#if error}<p class="err" role="alert">{error}</p>{/if}
    {#if store.boards.length}
      <ul class="list">
        {#each store.boards as b (b.id)}
          <li>
            <a
              class="card"
              href={boardPath(b.id)}
              onclick={(e) => {
                if (e.metaKey || e.ctrlKey || e.shiftKey || e.button) return;
                e.preventDefault();
                open(b.id);
              }}
            >
              <LayoutDashboard size={18} />
              <span class="grow">
                <strong>{b.name}</strong>
                {#if b.description}<span class="dim"> — {b.description}</span>{/if}
              </span>
              <span class="dim">{b.sections.length} {b.sections.length === 1 ? 'section' : 'sections'}</span>
            </a>
          </li>
        {/each}
      </ul>
    {:else}
      <p class="dim">No boards yet.</p>
    {/if}
  {:else}
    <div class="head row">
      <button class="ghost" onclick={() => (store.boardId = null)} aria-label="All boards" title="All boards"
        ><ChevronLeft size={18} /></button
      >
      <h1 class="grow">{draft ? 'Editing ' : ''}{(draft ?? board).name}</h1>
      {#if draft}
        <button onclick={cancel} disabled={busy}>Cancel</button>
        <button class="primary" onclick={save} disabled={busy || !valid}>Save</button>
      {:else}
        <button onclick={edit}><Pencil size={15} /> Edit</button>
        <button class="danger" onclick={remove} disabled={busy}
          ><Trash2 size={15} /> {confirmDelete ? 'Really delete?' : 'Delete'}</button
        >
      {/if}
    </div>
    {#if !draft}<p class="dim address"><code>{boardPath(board.id)}</code></p>{/if}
    {#if !draft && board.description}<p class="dim">{board.description}</p>{/if}
    {#if !draft && board.filter}<p class="dim">Filter: <code>{board.filter}</code></p>{/if}
    {#if error}<p class="err" role="alert">{error}</p>{/if}

    {#if draft}
      <section class="panel" aria-label="Board settings">
        <label>
          Name
          <input bind:value={draft.name} />
        </label>
        <label>
          Description
          <input bind:value={draft.description} placeholder="optional" />
        </label>
        <label class="slug">
          Address
          <span class="slug-input">
            <span class="dim">/board/</span>
            <input
              value={draft.id}
              oninput={(e) => draft && (draft.id = e.currentTarget.value.trim().toLowerCase())}
              spellcheck="false"
              autocomplete="off"
              aria-invalid={!!slugError}
              aria-describedby="slug-problem"
              class="mono"
            />
          </span>
          <span id="slug-problem" class:err={!!slugError} class="dim small">
            {slugError ?? 'The last part of the board’s address, so a link to it can be shared.'}
          </span>
        </label>
        <div class="wide filter">
          <span>Filter for every section</span>
          <FilterInput
            bind:value={() => draft?.filter ?? '', (v) => draft && (draft.filter = v || undefined)}
            id="board-filter"
            placeholder="optional: project:Home"
          />
        </div>
        <div class="wide row adds">
          <span class="dim">Add</span>
          {#each SECTION_TYPES as t (t.id)}
            <button onclick={() => add(t.id)}><Plus size={14} /> {t.label}</button>
          {/each}
        </div>
      </section>
      {#if problems.length}
        <ul class="err problems" role="alert">
          {#each problems as line, i (i)}<li>{line}</li>{/each}
        </ul>
      {/if}
      {#if picked}
        <section class="panel" aria-label="Section settings">
          <h2>{picked.title || 'Section'}</h2>
          <SectionForm section={picked} onchange={setSection} />
        </section>
      {/if}
    {/if}

    <BoardView
      board={draft ?? board}
      editing={!!draft}
      {selected}
      onchange={setSections}
      onselect={(id) => (selected = id)}
      onremove={removeSection}
    />
  {/if}
</div>

<style>
  .page {
    max-width: 1200px;
    margin: 0 auto;
  }
  h1 {
    font-size: 22px;
    margin: 0 0 10px;
  }
  .head h1 {
    margin: 0;
  }
  h2 {
    font-size: 15px;
    margin: 0 0 10px;
  }
  .head {
    margin-bottom: 10px;
    flex-wrap: wrap;
  }
  .new {
    margin: 10px 0 14px;
    max-width: 480px;
  }
  .new input {
    flex: 1;
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 8px;
    max-width: 640px;
  }
  .address {
    margin: 0 0 6px;
  }
  .slug-input {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .slug-input input {
    flex: 1;
  }
  .small {
    font-size: 12px;
  }
  .card {
    color: inherit;
    text-decoration: none;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    text-align: left;
    padding: 10px 12px;
  }
  .panel {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 10px 12px;
    background: var(--panel-2);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 12px;
    margin-bottom: 12px;
  }
  .panel h2 {
    grid-column: 1 / -1;
  }
  .panel label,
  .panel .filter {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 12px;
    color: var(--dim);
  }
  .panel input {
    font-size: 14px;
    color: var(--text);
  }
  .wide {
    grid-column: 1 / -1;
  }
  .adds {
    flex-wrap: wrap;
  }
  .problems {
    margin: 0 0 12px;
    padding-left: 20px;
  }
  .warn {
    display: flex;
    gap: 8px;
    margin-bottom: 12px;
    padding: 8px 12px;
    border: 1px solid var(--warn);
    border-radius: var(--radius);
    color: var(--warn);
  }
  .warn ul {
    margin: 4px 0 0;
    padding-left: 18px;
  }
</style>
