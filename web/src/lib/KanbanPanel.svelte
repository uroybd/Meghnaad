<script lang="ts">
  // Settings → Kanban (spec 0026). Off until set up, automatically (a UDA, written with `config` as the tasksh tab
  // writes its lines, and Kanban's report, filter and columns on it) or by hand (nothing in the taskrc). Then Kanban's
  // settings, which every kanban is drawn with, and the kanbans, each a name and an address and, where it has its own,
  // a report, filter, Completed since or columns; all kept in `settings.json` under `kanban`.
  import { untrack } from 'svelte';
  import { runCli } from './api';
  import { MAX_SETTINGS_BYTES, serializeKanban } from './appsettings';
  import { ArrowDown, ArrowUp, Plus, TriangleAlert, X } from './icons';
  import {
    automaticSettings,
    columnProblem,
    hasReview,
    kanbanProblem,
    newKanban,
    renamedKanban,
    resolveKanban,
    setupLines,
    reportLines,
    KANBAN_REPORT,
    udaNameProblem,
    udaStatus,
    type KanbanColumn,
    type KanbanEntry,
    type KanbanSettings,
    type Part,
  } from './kanban';
  import { store } from './store.svelte';
  import { configArgs } from './tasksh';

  /** There are edits not yet applied (the tab shows a dot). */
  let { ondirty }: { ondirty?: (dirty: boolean) => void } = $props();

  const copy = (k: KanbanSettings): KanbanSettings => JSON.parse(JSON.stringify(k));
  const same = (a: KanbanSettings, b: KanbanSettings) => JSON.stringify(a) === JSON.stringify(b);

  const saved = $derived(store.kanbanSettings);
  let draft = $state<KanbanSettings>(untrack(() => copy(saved)));
  let busy = $state(false);
  let error = $state<string | null>(null);
  let done = $state(false);

  const edited = $derived(!same(draft, saved));
  $effect(() => {
    ondirty?.(edited);
  });

  // The saved settings changed (applied here, settings.json read late, another tab): follow them, unless there are edits.
  let before = untrack(() => saved);
  $effect(() => {
    const now = saved;
    const untouched = same(
      untrack(() => draft),
      before,
    );
    const changed = !same(now, before);
    before = now;
    if (changed && untouched) draft = copy(now);
  });

  const udaProblem = $derived(udaNameProblem(draft.uda));
  const status = $derived(udaStatus(store.config, draft.uda));
  const lines = $derived(udaProblem ? [] : setupLines(store.config, draft.uda));
  // The automatic setup can add a `kanban` report to the taskrc, so the kanbans need no filter of their own.
  let withReport = $state(true);
  const hasReport = $derived(!!store.config?.config.reports?.[KANBAN_REPORT]);
  const addLines = $derived(withReport ? reportLines(store.config) : []);
  const review = $derived(hasReview(store.config));
  const label: Record<Part, string> = { ok: 'defined', missing: 'not defined', different: 'defined differently' };

  /** What is wrong with Kanban's columns, and with each kanban and its own columns, by index. */
  const shared = $derived(draft.columns.map(columnProblem));
  const problems = $derived(
    draft.kanbans.map((k, i) => ({
      kanban: kanbanProblem(
        resolveKanban(draft, k),
        draft.kanbans.filter((_, j) => j !== i).map((o) => o.id),
      ),
      columns: (k.columns ?? []).map(columnProblem),
    })),
  );
  const valid = $derived(
    !udaProblem && shared.every((c) => !c) && problems.every((p) => !p.kanban && p.columns.every((c) => !c)),
  );
  const tooLarge = $derived(new Blob([serializeKanban(store.appSettings?.raw ?? {}, draft)]).size > MAX_SETTINGS_BYTES);
  /** Entries in the saved file that were not understood: they stay in the file. */
  const unread = $derived(store.appSettings?.kanbanErrors ?? []);

  async function save(settings: KanbanSettings) {
    busy = true;
    error = null;
    done = false;
    try {
      await store.saveKanban(settings);
      draft = copy(settings);
      done = true;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  /** Write the UDA's `config` lines (and the report's, when the setup adds it); throws what the engine refused. */
  async function writeLines(more: [string, string][] = []) {
    try {
      for (const line of [...lines, ...more]) {
        const r = (await runCli({ args: configArgs(line) })).result;
        if (r.kind === 'error') throw new Error(r.message);
      }
    } finally {
      await store.settingsChanged();
    }
  }

  async function setUp(automatic: boolean) {
    if (automatic && udaProblem) return;
    busy = true;
    error = null;
    try {
      if (automatic) await writeLines(addLines);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
      busy = false;
      return;
    }
    busy = false;
    await save(
      automatic
        ? automaticSettings(draft, review, draft.uda, withReport)
        : { ...draft, enabled: true, setup: 'manual', report: store.reports[0]?.name ?? 'next' },
    );
  }

  async function applyLines() {
    busy = true;
    error = null;
    try {
      await writeLines();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  function addKanban() {
    draft.kanbans.push(
      newKanban(
        '',
        draft.kanbans.map((b) => b.id),
      ),
    );
    done = false;
  }

  /** Give `k` its own `key`, starting from Kanban's, or take it away. */
  function own(k: KanbanEntry, key: 'report' | 'filter' | 'completedSince' | 'columns') {
    if (k[key] !== undefined) delete k[key];
    else if (key === 'columns') k.columns = draft.columns.map((c) => ({ ...c }));
    else k[key] = draft[key];
    done = false;
  }

  function moveColumn(cols: KanbanColumn[], i: number, by: number) {
    const j = i + by;
    if (j < 0 || j >= cols.length) return;
    [cols[i], cols[j]] = [cols[j], cols[i]];
    done = false;
  }

  const OWN_LABEL = { report: 'report', filter: 'filter', completedSince: 'Completed since', columns: 'columns' };
</script>

{#snippet reportField(get: () => string, set: (v: string) => void)}
  <select value={get()} onchange={(e) => (set(e.currentTarget.value), (done = false))}>
    {#if !store.reports.some((r) => r.name === get())}<option value={get()}>{get()}</option>{/if}
    {#each store.reports as r (r.name)}<option value={r.name}>{r.name}</option>{/each}
  </select>
{/snippet}

{#snippet textField(get: () => string, set: (v: string) => void, placeholder: string)}
  <input
    class="mono"
    value={get()}
    oninput={(e) => (set(e.currentTarget.value), (done = false))}
    {placeholder}
    spellcheck="false"
    autocomplete="off"
    autocapitalize="off"
  />
{/snippet}

{#snippet columnsEditor(cols: KanbanColumn[], why: (string | null)[])}
  {#if cols.length}
    <ol class="cols">
      {#each cols as c, j (j)}
        <li class="col" class:invalid={!!why[j]}>
          <input bind:value={c.name} oninput={() => (done = false)} aria-label="Column name" placeholder="Name" />
          <input
            class="mono"
            bind:value={c.filter}
            oninput={() => (done = false)}
            aria-label="Filter of {c.name || 'this column'}"
            placeholder="Filter: +doing"
            spellcheck="false"
            autocomplete="off"
            autocapitalize="off"
          />
          <input
            class="mono"
            bind:value={c.move}
            oninput={() => (done = false)}
            aria-label="On move of {c.name || 'this column'}"
            placeholder="On move: modify +doing (empty: takes no cards)"
            spellcheck="false"
            autocomplete="off"
            autocapitalize="off"
          />
          <span class="tools">
            <button
              class="ghost"
              aria-label="Move {c.name || 'this column'} left"
              title="Earlier"
              disabled={j === 0}
              onclick={() => moveColumn(cols, j, -1)}><ArrowUp size={14} /></button
            >
            <button
              class="ghost"
              aria-label="Move {c.name || 'this column'} right"
              title="Later"
              disabled={j === cols.length - 1}
              onclick={() => moveColumn(cols, j, 1)}><ArrowDown size={14} /></button
            >
            <button
              class="ghost"
              aria-label="Remove {c.name || 'this column'}"
              title="Remove"
              onclick={() => (cols.splice(j, 1), (done = false))}><X size={14} /></button
            >
          </span>
          {#if why[j]}<span class="why" role="alert">{why[j]}</span>{/if}
        </li>
      {/each}
    </ol>
  {:else}
    <p class="dim">No columns yet.</p>
  {/if}
  <div class="row">
    <button class="btn" onclick={() => (cols.push({ name: '', filter: '', move: '' }), (done = false))}
      ><Plus size={14} /> Add a column</button
    >
  </div>
{/snippet}

<section aria-label="Kanban settings">
  <h2>Kanban</h2>
  <p class="dim">
    A kanban is columns of tasks, left to right. Each column has a <strong>filter</strong> that picks its tasks (a task
    is in the first column that matches it) and an <strong>On move</strong> command line, run on a task dropped into it:
    <code>modify {draft.uda}:review</code>, <code>done</code>, or one of your commands. A kanban is drawn from a
    <strong>report</strong>, whose filter, order and columns it takes: what a card shows is that report's columns. The
    settings below are every kanban's; a kanban can have its own report, filter, Completed since or columns instead.
  </p>

  {#if !saved.enabled}
    <div class="choices">
      <div class="choice">
        <h3>Automatic</h3>
        <p class="dim">
          Defines a UDA and gives every kanban the columns To do, In progress, Review and Done. Moving a card out of
          Done reopens it; nothing starts a task.
          {#if review}
            tasksh's review is set up, so moving a card to Review puts it back in the review queue (it clears
            <code>reviewed</code>).
          {/if}
        </p>
        <div class="fields">
          <label
            >UDA {@render textField(
              () => draft.uda,
              (v) => (draft.uda = v),
              'progressState',
            )}</label
          >
        </div>
        {#if udaProblem}
          <p class="why" role="alert">{udaProblem}</p>
        {:else}
          <p>
            The <code>{draft.uda}</code> UDA is <strong>{label[status.part]}</strong>{#if status.part === 'different'}
              (type <code>{status.type}</code>{#if status.values.length}, values <code>{status.values.join(',')}</code
                >{/if}){/if}.
          </p>
          <label class="check"
            ><input type="checkbox" bind:checked={withReport} /> Draw the kanbans on a
            <code>{KANBAN_REPORT}</code> report{hasReport
              ? ' (your taskrc has one: it is used as it is)'
              : ', added to your taskrc'}</label
          >
          <p class="dim">
            {#if withReport}Its filter leaves out deleted and waiting tasks and recurring templates, so a kanban needs
              no filter of its own.{:else}The kanbans are drawn on <code>all</code>, with the filter
              <code>( +PENDING or +COMPLETED )</code>.{/if}
          </p>
          {#if lines.length || addLines.length}
            <p class="dim">Set up writes these lines to your taskrc, and nothing else:</p>
            <pre class="lines">{[...lines, ...addLines].map(([n, v]) => `config ${n} ${v}`).join('\n')}</pre>
          {/if}
        {/if}
        <button class="primary" disabled={busy || !!udaProblem} onclick={() => setUp(true)}>Set up automatically</button
        >
      </div>
      <div class="choice">
        <h3>Manual</h3>
        <p class="dim">
          Nothing in your taskrc changes: you write the columns yourself, with tags (<code>+next</code>), a UDA of your
          own, <code>+ACTIVE</code>, anything a filter can say.
        </p>
        <button class="primary" disabled={busy} onclick={() => setUp(false)}>Turn on</button>
      </div>
    </div>
  {:else}
    {#if status.part !== 'ok' && !udaProblem && saved.setup === 'automatic' && draft.uda === saved.uda}
      <div class="bad" role="alert">
        <TriangleAlert size={15} />
        <div>
          The <code>{draft.uda}</code> UDA is {label[status.part]}. These lines would define it:
          <pre class="lines">{lines.map(([n, v]) => `config ${n} ${v}`).join('\n')}</pre>
          <button class="btn" disabled={busy} onclick={applyLines}>Write them</button>
        </div>
      </div>
    {/if}

    {#if unread.length}
      <div class="bad" role="alert">
        <TriangleAlert size={15} />
        <div>
          <strong
            >{unread.length === 1 ? 'One entry' : `${unread.length} entries`} in settings.json can't be used</strong
          >
          and {unread.length === 1 ? 'is' : 'are'} kept in it as {unread.length === 1 ? 'it is' : 'they are'}:
          <ul>
            {#each unread as why (why)}<li>{why}</li>{/each}
          </ul>
        </div>
      </div>
    {/if}

    <fieldset class="kanban">
      <legend>Every kanban</legend>
      <div class="fields">
        <label
          >UDA {@render textField(
            () => draft.uda,
            (v) => (draft.uda = v),
            'progressState',
          )}
          {#if udaProblem}<span class="why" role="alert">{udaProblem}</span>{/if}</label
        >
        <label
          >Report {@render reportField(
            () => draft.report,
            (v) => (draft.report = v),
          )}</label
        >
        <label
          >Filter {@render textField(
            () => draft.filter,
            (v) => (draft.filter = v),
            'project:Work',
          )}</label
        >
        <label
          >Completed since {@render textField(
            () => draft.completedSince,
            (v) => (draft.completedSince = v),
            'today-14d (empty: all)',
          )}</label
        >
      </div>
      <label class="check"
        ><input type="checkbox" bind:checked={draft.icons} /> Show the fields on cards as icons, not their report labels (a
        UDA as a badge: its label, then its value)</label
      >
      {#if draft.uda !== saved.uda}
        <p class="dim">The columns name the UDA in their own filters and moves: change them to match.</p>
      {/if}
      <h4>Columns</h4>
      {@render columnsEditor(draft.columns, shared)}
    </fieldset>

    {#each draft.kanbans as k, i (i)}
      {@const p = problems[i]}
      <fieldset class="kanban">
        <legend>{k.name || 'New kanban'}</legend>
        <div class="fields">
          <label
            >Name <input
              value={k.name}
              oninput={(e) => {
                draft.kanbans[i] = renamedKanban(
                  k,
                  (e.currentTarget as HTMLInputElement).value,
                  saved.kanbans.some((s) => s.id === k.id),
                  draft.kanbans.filter((_, j) => j !== i).map((o) => o.id),
                );
                done = false;
              }}
              placeholder="Work"
            /></label
          >
          <label
            >Address <span class="addr"
              ><span class="dim mono">/kanban/</span><input
                class="mono"
                bind:value={k.id}
                oninput={() => (done = false)}
                spellcheck="false"
                autocomplete="off"
                autocapitalize="off"
              /></span
            ></label
          >
        </div>
        <div class="owns">
          {#each ['report', 'filter', 'completedSince', 'columns'] as const as key (key)}
            <label class="own"
              ><input type="checkbox" checked={k[key] !== undefined} onchange={() => own(k, key)} /> Its own {OWN_LABEL[
                key
              ]}</label
            >
          {/each}
        </div>
        {#if k.report !== undefined || k.filter !== undefined || k.completedSince !== undefined}
          <div class="fields">
            {#if k.report !== undefined}
              <label
                >Report {@render reportField(
                  () => k.report ?? '',
                  (v) => (k.report = v),
                )}</label
              >
            {/if}
            {#if k.filter !== undefined}
              <label
                >Filter {@render textField(
                  () => k.filter ?? '',
                  (v) => (k.filter = v),
                  'project:Work',
                )}</label
              >
            {/if}
            {#if k.completedSince !== undefined}
              <label
                >Completed since {@render textField(
                  () => k.completedSince ?? '',
                  (v) => (k.completedSince = v),
                  'today-14d (empty: all)',
                )}</label
              >
            {/if}
          </div>
        {/if}
        {#if p?.kanban}<p class="why" role="alert">{p.kanban}</p>{/if}
        {#if k.columns}
          <h4>Its columns</h4>
          {@render columnsEditor(k.columns, p?.columns ?? [])}
        {/if}
        <div class="row">
          <span class="grow"></span>
          <button class="ghost danger" onclick={() => (draft.kanbans.splice(i, 1), (done = false))}
            >Delete this kanban</button
          >
        </div>
      </fieldset>
    {/each}

    <div class="row">
      <button class="btn" onclick={addKanban}><Plus size={14} /> Add a kanban</button>
    </div>

    {#if tooLarge}<p class="err" role="alert">
        That is more than {MAX_SETTINGS_BYTES / 1024} KB, which is the most that can be saved.
      </p>{/if}
    {#if done && !edited}<p class="ok" role="status">Applied.</p>{/if}

    <footer class="row">
      <button
        class="ghost"
        disabled={busy}
        title="Hide the Kanban page; the kanbans stay in settings.json, and the UDA in your taskrc"
        onclick={() => save({ ...saved, enabled: false })}>Turn Kanban off</button
      >
      <span class="grow"></span>
      {#if edited}<button class="ghost" onclick={() => ((draft = copy(saved)), (done = false))}>Revert edits</button
        >{/if}
      <button class="primary" onclick={() => save(draft)} disabled={busy || !edited || !valid || tooLarge}>Apply</button
      >
    </footer>
  {/if}
  {#if error}<p class="err" role="alert">{error}</p>{/if}
</section>

<style>
  .check {
    display: flex;
    align-items: baseline;
    gap: 6px;
    margin: 8px 0 2px;
  }
  h2 {
    margin: 0 0 4px;
    font-size: 18px;
  }
  h3 {
    margin: 0 0 6px;
    font-size: 15px;
  }
  h4 {
    margin: 12px 0 6px;
    font-size: 13px;
  }
  code {
    font-family: var(--mono);
    background: var(--panel-2);
    border-radius: 4px;
    padding: 0 4px;
  }
  .choices {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
    gap: 12px;
    margin-top: 12px;
  }
  .choice {
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 12px;
  }
  .lines {
    font-family: var(--mono);
    background: var(--panel-2);
    border-radius: 6px;
    padding: 8px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    font-size: 12.5px;
  }
  .bad {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    color: var(--err);
    margin: 10px 0;
  }
  .bad ul {
    margin: 4px 0 0;
    padding-left: 18px;
  }
  .kanban {
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 10px 12px;
    margin: 14px 0;
    min-width: 0;
  }
  legend {
    font-weight: 600;
    padding: 0 4px;
  }
  .fields {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 8px 12px;
  }
  .fields label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 13px;
    min-width: 0;
  }
  .owns {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 14px;
    margin: 10px 0 4px;
    font-size: 13px;
  }
  .own {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  .addr {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .addr input {
    flex: 1;
    min-width: 0;
  }
  .cols {
    margin: 0;
    padding-left: 22px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .col {
    display: grid;
    grid-template-columns: minmax(90px, 140px) 1fr 1fr auto;
    gap: 6px;
    align-items: center;
  }
  .col input {
    min-width: 0;
  }
  .col.invalid input {
    border-color: var(--err);
  }
  .tools {
    display: inline-flex;
  }
  .why {
    grid-column: 1 / -1;
    color: var(--err);
    font-size: 12.5px;
    margin: 4px 0 0;
  }
  .btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    margin-top: 10px;
  }
  .grow {
    flex: 1;
  }
  footer {
    margin-top: 16px;
  }
  @media (max-width: 760px) {
    .col {
      grid-template-columns: 1fr auto;
    }
    .col input {
      grid-column: 1;
    }
    .tools {
      grid-column: 2;
      grid-row: 1;
    }
  }
</style>
