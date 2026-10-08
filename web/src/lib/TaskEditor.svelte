<script lang="ts">
  import DateTimeInput from './DateTimeInput.svelte';
  import { formatFor } from './dateformat';
  import { formatMoment, fromParts, toParts } from './dates';
  import { udaLabel } from './format';
  import { newlineOnEnter, submitOnEnter } from './multiline';
  import { CornerUpLeft, Lock, Repeat, RotateCcw, X } from './icons';
  import { visibleNotes } from './journal';
  import { describeRecur, presetFor, PRESETS } from './recurrence';
  import { store, type Entry } from './store.svelte';
  import type { Row, UdaDef } from './types';

  // One form for both jobs: `row` set = edit that task, `row` null = create a new one.
  // Either way it only sends what changed, as `modify`/`add` arguments.
  let {
    row = null,
    from = null,
    initialDescription = '',
    onclose,
    onadded,
  }: {
    row?: Row | null;
    from?: Entry | null;
    initialDescription?: string;
    onclose: () => void;
    /** Add mode: "Add and another" asks the parent to re-mount a fresh form. */
    onadded?: (again: boolean) => void;
  } = $props();

  // The form is re-mounted per task (keyed in App), so reading `row` once is intended.
  /* svelte-ignore state_referenced_locally */
  const adding = row === null;
  const defs = $derived(store.config?.config.udas ?? {});
  const defined = $derived(Object.values(defs));
  const priorityValues = $derived(
    defs.priority?.values.length ? defs.priority.values.filter(Boolean) : ['H', 'M', 'L'],
  );

  const wire = (ts: number | null | undefined) => fromParts(toParts(ts ?? null));
  function uda(u: UdaDef, r: Row | null): string {
    if (!r) return u.default ?? '';
    const raw = r.extra[u.name] ?? '';
    return u.type === 'date' && raw ? wire(Number(raw)) : raw;
  }

  // What the fields start as. Edit diffs against the task; add diffs against "empty" (plus UDA defaults).
  /* svelte-ignore state_referenced_locally */
  const start = {
    description: row?.description ?? initialDescription,
    project: row?.project ?? '',
    priority: row?.priority ?? '',
    due: wire(row?.due),
    wait: wire(row?.wait),
    scheduled: wire(row?.scheduled),
    until: wire(row?.until),
    tags: (row?.tags ?? []).join(' '),
    recur: row?.recur ?? '',
  };
  /* svelte-ignore state_referenced_locally */
  const udaStart = Object.fromEntries(Object.values(store.config?.config.udas ?? {}).map((u) => [u.name, uda(u, row)]));

  /* svelte-ignore state_referenced_locally */
  let description = $state(start.description);
  /* svelte-ignore state_referenced_locally */
  let project = $state(start.project);
  /* svelte-ignore state_referenced_locally */
  let priority = $state(start.priority);
  /* svelte-ignore state_referenced_locally */
  let due = $state(start.due);
  /* svelte-ignore state_referenced_locally */
  let wait = $state(start.wait);
  /* svelte-ignore state_referenced_locally */
  let scheduled = $state(start.scheduled);
  /* svelte-ignore state_referenced_locally */
  let until = $state(start.until);
  /* svelte-ignore state_referenced_locally */
  let tags = $state(start.tags);
  // Repeat: a preset, or "custom" with a free-text period (`3d`, `P2W`, ...). The server validates it.
  /* svelte-ignore state_referenced_locally */
  let recurChoice = $state(start.recur === '' ? '' : presetFor(start.recur) || 'custom');
  /* svelte-ignore state_referenced_locally */
  let recurCustom = $state(presetFor(start.recur) ? '' : start.recur);
  const recur = $derived(recurChoice === 'custom' ? recurCustom.trim() : recurChoice);
  // Instances follow their template: its period is shown but changed on the template.
  /* svelte-ignore state_referenced_locally */
  const instance = row?.parent != null;
  // A template can't stop repeating (Taskwarrior refuses); delete it instead.
  /* svelte-ignore state_referenced_locally */
  const template = row?.status === 'recurring';
  const needsDue = $derived(!instance && recur !== '' && due === '');
  /* svelte-ignore state_referenced_locally */
  let udaValues = $state<Record<string, string>>({ ...udaStart });
  let startNow = $state(false);
  let removedDeps = $state<string[]>([]);
  let newDep = $state('');
  let note = $state('');
  let busy = $state(false);
  let dialog: HTMLDialogElement;

  function changes(): string[] {
    const a: string[] = [];
    const set = (name: string, now: string, was: string) => {
      if (now !== was) a.push(`${name}:${now}`);
    };
    if (description.trim() !== start.description) a.push(`description:${description.trim()}`);
    set('project', project.trim(), start.project);
    set('priority', priority, start.priority);
    set('due', due, start.due);
    set('wait', wait, start.wait);
    set('scheduled', scheduled, start.scheduled);
    set('until', until, start.until);
    if (!instance) set('recur', recur, start.recur);
    const want = new Set(tags.split(/[\s,]+/).filter(Boolean).map((t) => t.replace(/^\+/, '')));
    const had = new Set(start.tags.split(' ').filter(Boolean));
    for (const t of want) if (!had.has(t)) a.push(`+${t}`);
    for (const t of had) if (!want.has(t)) a.push(`-${t}`);
    for (const u of defined) set(u.name, udaValues[u.name] ?? '', udaStart[u.name] ?? '');
    for (const d of removedDeps) a.push(`depends:-${d}`);
    for (const d of newDep.split(/[\s,]+/).filter(Boolean)) a.push(`depends:${d}`);
    // Starting is a separate step after adding (see save): the `start` command also journals.
    return a;
  }

  const dirty = $derived((adding ? description.trim().length > 0 : changes().length > 0) && !needsDue);
  const command = $derived(
    adding
      ? ['add', ...changes()].join(' ') + (startNow ? '  then: start' : '')
      : `${row!.uuid.slice(0, 8)} modify ${changes().join(' ')}`,
  );

  async function save(again = false) {
    if (!dirty) return onclose();
    busy = true;
    const res = await store.act(from, adding ? ['add', ...changes()] : [row!.uuid, 'modify', ...changes()]);
    if (adding && res?.result.kind === 'changed') {
      const uuid = res.result.tasks[0].uuid;
      if (startNow) await store.act(from, [uuid, 'start']);
      if (note.trim()) await store.act(from, [uuid, 'annotate', note.trim()]);
    }
    busy = false;
    if (res && res.result.kind !== 'error') {
      if (adding && onadded) onadded(again);
      else onclose();
    }
  }

  async function annotate() {
    if (!row || !note.trim()) return;
    const res = await store.act(from, [row.uuid, 'annotate', note.trim()]);
    if (res && res.result.kind !== 'error') {
      note = '';
      onclose();
    }
  }

  async function denotate(text: string) {
    if (!row) return;
    const res = await store.act(from, [row.uuid, 'denotate', text]);
    if (res && res.result.kind !== 'error') onclose();
  }

  $effect(() => {
    dialog.showModal();
  });
</script>

<dialog bind:this={dialog} onclose={onclose} aria-label={adding ? 'New task' : 'Edit task'}>
  <form method="dialog" onsubmit={(e) => { e.preventDefault(); save(false); }}>
    <h3>
      {adding ? 'New task' : 'Edit task'}
      {#if row}<span class="dim mono">{row.uuid.slice(0, 8)}</span>{/if}
    </h3>

    <label for="te-desc">Description</label>
    <input id="te-desc" bind:value={description} required />

    <label for="te-proj">Project</label>
    <input id="te-proj" bind:value={project} list="te-projects" placeholder="none" />
    <datalist id="te-projects">{#each store.projects as p}<option value={p}></option>{/each}</datalist>

    <label for="te-pri">Priority</label>
    <select id="te-pri" bind:value={priority}>
      <option value="">none</option>
      {#each priorityValues as p}<option value={p}>{p}</option>{/each}
    </select>

    <label for="te-due">Due</label><DateTimeInput id="te-due" label="Due" bind:value={due} />
    <label for="te-wait">Wait</label><DateTimeInput id="te-wait" label="Wait" bind:value={wait} />
    <label for="te-sched">Scheduled</label><DateTimeInput id="te-sched" label="Scheduled" bind:value={scheduled} />
    <label for="te-until">Until</label><DateTimeInput id="te-until" label="Until" bind:value={until} />

    <label for="te-recur">Repeat</label>
    {#if instance}
      <div>
        <span class="chip"><Repeat size={12} /> {describeRecur(row?.recur)}</span>
        <span class="dim hint">This task is one instance of a series.</span>
        {#if row?.parent}
          <button type="button" class="ghost link" onclick={() => { const p = row!.parent!; onclose(); store.openDetail(p, from); }}>
            <CornerUpLeft size={13} /> open the recurring task
          </button>
        {/if}
      </div>
    {:else}
      <div class="repeat">
        <div class="row wrapline">
          <select id="te-recur" bind:value={recurChoice} aria-label="Repeat">
            {#if !template}<option value="">Doesn't repeat</option>{/if}
            {#each PRESETS as p}<option value={p.value}>{p.label}</option>{/each}
            <option value="custom">Custom…</option>
          </select>
          {#if recurChoice === 'custom'}
            <input
              bind:value={recurCustom}
              aria-label="Custom repeat period"
              placeholder="3d, 2w, P1M, 6mo…"
              autocapitalize="off"
              spellcheck="false"
            />
          {/if}
        </div>
        {#if needsDue}
          <div class="err hint" role="alert">A repeating task needs a due date.</div>
        {:else if recur && !template}
          <div class="dim hint">Repeats {describeRecur(recur)}, starting from the due date.</div>
        {:else if template}
          <div class="dim hint">Changes to a repeating task also apply to its open instances (not their dates).</div>
        {/if}
      </div>
    {/if}

    <label for="te-tags">Tags</label>
    <input id="te-tags" bind:value={tags} list="te-tag-list" placeholder="space separated" />
    <datalist id="te-tag-list">{#each store.tags as t}<option value={t}></option>{/each}</datalist>

    {#each defined as u (u.name)}
      <label for="te-uda-{u.name}">{udaLabel(u)}</label>
      {#if u.type === 'date'}
        <DateTimeInput id="te-uda-{u.name}" label={udaLabel(u)} bind:value={udaValues[u.name]} />
      {:else if u.type === 'string' && u.values.length}
        <select id="te-uda-{u.name}" bind:value={udaValues[u.name]}>
          <option value="">none</option>
          {#each u.values.filter(Boolean) as v}<option value={v}>{v}</option>{/each}
        </select>
      {:else if u.type === 'numeric'}
        <input id="te-uda-{u.name}" type="number" step="any" bind:value={udaValues[u.name]} />
      {:else if u.type === 'string'}
        <textarea id="te-uda-{u.name}" rows="1" bind:value={udaValues[u.name]} onkeydown={newlineOnEnter} title="Enter adds a line; Ctrl+Enter saves"></textarea>
      {:else}
        <input id="te-uda-{u.name}" bind:value={udaValues[u.name]} />
      {/if}
    {/each}

    <label for="te-dep">Depends on</label>
    <div>
      {#each row?.depends ?? [] as d}
        <span class="chip" class:gone={removedDeps.includes(d)}>
          {d.slice(0, 8)}
          <button
            type="button" class="ghost x"
            aria-label={removedDeps.includes(d) ? 'Keep dependency' : 'Remove dependency'}
            onclick={() => (removedDeps = removedDeps.includes(d) ? removedDeps.filter((x) => x !== d) : [...removedDeps, d])}
          >{#if removedDeps.includes(d)}<RotateCcw size={12} />{:else}<X size={12} />{/if}</button>
        </span>
      {/each}
      <input id="te-dep" bind:value={newDep} list="te-task-list" placeholder="task ids or uuid prefixes" />
      <datalist id="te-task-list">
        {#each store.tasks.filter((t) => t.id != null) as t}<option value={String(t.id)}>{t.description}</option>{/each}
      </datalist>
    </div>

    {#if adding}
      <span class="lbl">Start</span>
      <label class="check"><input type="checkbox" bind:checked={startNow} /> start working on it now</label>
    {/if}

    {#if row?.orphans.length}
      <span class="lbl"><Lock size={13} /> Not in taskrc</span>
      <div class="orphans" title="These properties exist on the task but aren't defined as UDAs in your taskrc, so they can't be edited here.">
        {#each row.orphans as k}
          <div class="mono"><span class="dim">{k}</span> = {row.extra[k]}</div>
        {/each}
        <div class="dim hint">read-only: define uda.{row.orphans[0]}.type in your taskrc to edit</div>
      </div>
    {/if}

    <span class="lbl">{adding ? 'Note' : 'Annotations'}</span>
    <div>
      {#each visibleNotes(row?.annotations ?? [], store.config?.journal) as a}
        <div class="ann">
          <span class="dim">{formatMoment(a.entry, undefined, formatFor('infoNote', store.config?.config.settings))}</span> <span class="text">{a.text}</span>
          <button type="button" class="ghost x" aria-label="Remove annotation" onclick={() => denotate(a.text)}><X size={12} /></button>
        </div>
      {/each}
      <div class="row">
        <textarea class="grow" rows="1" bind:value={note} onkeydown={submitOnEnter} placeholder={adding ? 'Optional first note' : 'Add an annotation'} aria-label="Annotation" title="Shift+Enter adds a line"></textarea>
        {#if !adding}<button type="button" onclick={annotate} disabled={!note.trim()}>Add</button>{/if}
      </div>
    </div>

    <footer class="row">
      <span class="grow dim mono preview" title="The command that will run">{#if dirty}task {command}{/if}</span>
      <button type="button" onclick={onclose}>Cancel</button>
      {#if adding}
        <button type="button" disabled={busy || !dirty} onclick={() => save(true)}>Add &amp; another</button>
      {/if}
      <button type="submit" class="primary" disabled={busy || !dirty}>{adding ? 'Add task' : 'Save'}</button>
    </footer>
  </form>
</dialog>

<style>
  dialog { border: 1px solid var(--line); border-radius: var(--radius); background: var(--panel); color: var(--text); padding: 24px 28px; width: min(760px, 96vw); max-height: 94vh; overflow: auto; }
  dialog::backdrop { background: rgb(0 0 0 / 0.4); }
  form { display: grid; grid-template-columns: 8.5em 1fr; gap: 12px 16px; align-items: start; }
  h3 { grid-column: 1 / -1; margin: 0 0 4px; }
  label, .lbl { color: var(--dim); padding-top: 5px; }
  label.check { padding-top: 5px; color: var(--text); }
  footer { grid-column: 1 / -1; margin-top: 8px; position: sticky; bottom: -16px; background: var(--panel); padding: 8px 0; }
  .preview { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; }
  .x { padding: 0 4px; font-size: 12px; display: inline-flex; vertical-align: middle; }
  .chip { display: inline-flex; align-items: center; gap: 2px; }
  .gone { opacity: 0.5; text-decoration: line-through; }
  .ann { margin-bottom: 4px; }
  .ann .text { white-space: pre-wrap; overflow-wrap: anywhere; }
  textarea { resize: vertical; field-sizing: content; min-height: 2.2em; max-height: 14em; }
  .orphans { opacity: 0.8; }
  .hint { font-size: 12px; }
  .link { display: inline-flex; align-items: center; gap: 4px; padding: 0 4px; }
  .wrapline { flex-wrap: wrap; }
  select, input:not([type='checkbox']) { max-width: 100%; }
  .repeat { display: grid; gap: 4px; }
  /* Phones: labels sit above their fields, and the buttons stack under the command preview. */
  @media (max-width: 760px) {
    form { grid-template-columns: minmax(0, 1fr); gap: 4px; }
    label, .lbl { padding-top: 8px; font-size: 12px; text-transform: uppercase; letter-spacing: 0.04em; }
    label.check { text-transform: none; font-size: inherit; display: flex; align-items: center; gap: 8px; padding-top: 4px; }
    footer { flex-wrap: wrap; bottom: calc(-1 * max(14px, env(safe-area-inset-bottom))); padding-bottom: max(8px, env(safe-area-inset-bottom)); border-top: 1px solid var(--line); }
    footer .preview { flex-basis: 100%; }
    footer button { flex: 1 1 auto; white-space: nowrap; }
    h3 { position: sticky; top: calc(-1 * max(14px, env(safe-area-inset-top))); background: var(--panel); padding: 6px 0; z-index: 1; }
  }
</style>
