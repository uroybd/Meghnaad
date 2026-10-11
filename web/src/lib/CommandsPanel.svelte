<script lang="ts">
  import { untrack } from 'svelte';
  import { putAppSettings } from './api';
  import { MAX_SETTINGS_BYTES, nameProblem, serializeSettings, type TaskCommand } from './appsettings';
  import IconPicker from './IconPicker.svelte';
  import { lucide } from './lucideLib.svelte';
  import { Plus, TriangleAlert, X } from './icons';
  import { store } from './store.svelte';

  /** There are edits not yet applied (the tab shows a dot). */
  let { ondirty }: { ondirty?: (dirty: boolean) => void } = $props();

  interface Row {
    key: number;
    name: string;
    command: string;
    icon: string;
  }
  let next = 0;
  const toRows = (cs: TaskCommand[]): Row[] =>
    cs.map((c) => ({ key: next++, name: c.name, command: c.command, icon: c.icon ?? '' }));
  const toCommands = (rows: Row[]): TaskCommand[] =>
    rows.map((r) =>
      r.icon
        ? { name: r.name.trim(), command: r.command.trim(), icon: r.icon }
        : { name: r.name.trim(), command: r.command.trim() },
    );
  const same = (a: TaskCommand[], b: TaskCommand[]) => JSON.stringify(a) === JSON.stringify(b);

  const saved = $derived(store.taskCommands);
  let rows = $state<Row[]>(untrack(() => toRows(saved)));
  let busy = $state(false);
  let error = $state<string | null>(null);
  let done = $state(false);

  const commands = $derived(toCommands(rows));
  const edited = $derived(!same(commands, saved));
  $effect(() => {
    ondirty?.(edited);
  });

  // The saved commands changed (this page applied them, or settings.json was read late): follow them, unless
  // there are edits here.
  let before = untrack(() => saved);
  $effect(() => {
    const now = saved;
    const untouched = same(
      untrack(() => commands),
      before,
    );
    const changed = !same(now, before);
    before = now;
    if (changed && untouched) rows = toRows(now);
  });

  /** What is wrong with each row, by key. */
  const problems = $derived.by(() => {
    const out = new Map<number, string>();
    const seen = new Set<string>();
    for (const r of rows) {
      const name = r.name.trim();
      const why = nameProblem(name) ?? (r.command.trim() ? null : 'needs a command');
      const dup = seen.has(name) ? `there is already a command named ${name}` : null;
      seen.add(name);
      const bad = why ?? dup ?? (r.icon && lucide.missing[r.icon] ? `no Lucide icon is called “${r.icon}”` : null);
      if (bad) out.set(r.key, bad);
    }
    return out;
  });
  const text = $derived(serializeSettings(store.appSettings?.raw ?? {}, commands));
  const tooLarge = $derived(new Blob([text]).size > MAX_SETTINGS_BYTES);
  const valid = $derived(problems.size === 0 && !tooLarge);
  /** Entries in the saved file that were not understood: applying drops them. */
  const unread = $derived(store.appSettings?.errors ?? []);

  function add() {
    rows.push({ key: next++, name: '', command: '', icon: '' });
    done = false;
  }

  async function apply() {
    busy = true;
    error = null;
    done = false;
    try {
      await putAppSettings(text);
      await store.loadAppSettings();
      done = true;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }
</script>

<section aria-label="Commands">
  <h2>Commands</h2>
  <p class="dim">
    A command is a word that stands for a whole command line. <code>fail</code> for <code>done outcome:fail</code>, and
    you can type <code>fail</code>, <code>task fail</code>, <code>3 fail</code> or <code>project:Home fail</code>: what
    comes before it selects the tasks. They also appear under the <strong>⋯</strong> menu on a task, and on the tasks
    you have selected. They are kept in this app's own <code>settings.json</code>, not in your taskrc. A command wins
    over a Taskwarrior command or report with the same name.
  </p>

  {#if unread.length}
    <div class="bad" role="alert">
      <TriangleAlert size={15} />
      <div>
        <strong>{unread.length === 1 ? 'One entry' : `${unread.length} entries`} in settings.json can't be used</strong>
        and will be dropped when you apply:
        <ul>
          {#each unread as why (why)}<li>{why}</li>{/each}
        </ul>
      </div>
    </div>
  {/if}

  {#if rows.length}
    <ul class="rows" aria-label="Commands">
      {#each rows as r (r.key)}
        {@const why = problems.get(r.key)}
        <li class="row-item" class:invalid={!!why}>
          <input
            class="name mono"
            bind:value={r.name}
            oninput={() => (done = false)}
            aria-label="Name"
            placeholder="fail"
            spellcheck="false"
            autocomplete="off"
            autocapitalize="off"
          />
          <input
            class="cmd mono"
            bind:value={r.command}
            oninput={() => (done = false)}
            aria-label="Command"
            placeholder="done outcome:fail"
            spellcheck="false"
            autocomplete="off"
            autocapitalize="off"
          />
          <IconPicker bind:value={r.icon} label="Icon for {r.name || 'the new command'}" />
          <button
            type="button"
            class="ghost"
            aria-label="Remove {r.name || 'this command'}"
            title="Remove"
            onclick={() => ((rows = rows.filter((x) => x.key !== r.key)), (done = false))}><X size={15} /></button
          >
          {#if why}<span class="why" role="alert">{why}</span>{/if}
        </li>
      {/each}
    </ul>
  {:else}
    <p class="dim">No commands yet.</p>
  {/if}

  <button type="button" class="btn" onclick={add}><Plus size={14} /> Add a command</button>

  <details class="json">
    <summary class="dim">settings.json</summary>
    <pre class="mono">{text}</pre>
  </details>

  {#if tooLarge}<p class="err" role="alert">
      That is more than {MAX_SETTINGS_BYTES / 1024} KB, which is the most that can be saved.
    </p>{/if}
  {#if error}<p class="err" role="alert">{error}</p>{/if}
  {#if done && !edited}<p class="ok" role="status">Applied.</p>{/if}

  <footer class="row">
    {#if edited}<button class="ghost" onclick={() => ((rows = toRows(saved)), (done = false))}>Revert edits</button
      >{/if}
    <span class="grow"></span>
    <button class="primary" onclick={apply} disabled={busy || !edited || !valid}>Apply</button>
  </footer>
</section>

<style>
  h2 {
    margin: 0 0 4px;
    font-size: 18px;
  }
  code {
    font-family: var(--mono);
    background: var(--panel-2);
    border-radius: 4px;
    padding: 0 4px;
  }
  .bad {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    color: var(--err);
  }
  .bad ul {
    margin: 4px 0 0;
    padding-left: 18px;
  }
  .rows {
    list-style: none;
    margin: 14px 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .row-item {
    position: relative;
    display: grid;
    grid-template-columns: minmax(90px, 160px) 1fr auto auto;
    gap: 8px;
    align-items: center;
  }
  .row-item input {
    min-width: 0;
  }
  .row-item.invalid input {
    border-color: var(--err);
  }
  .why {
    grid-column: 1 / -1;
    color: var(--err);
    font-size: 12.5px;
  }
  .btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  .json {
    margin: 14px 0;
  }
  .json pre {
    margin: 6px 0 0;
    padding: 10px 12px;
    background: var(--panel-2);
    border-radius: var(--radius);
    font-size: 12.5px;
    overflow: auto;
  }
  footer {
    margin-top: 16px;
    gap: 8px;
  }
  @media (max-width: 760px) {
    .row-item {
      grid-template-columns: 1fr auto auto;
    }
    .row-item .name {
      grid-column: 1 / -1;
    }
    .row-item .cmd {
      grid-column: 1;
    }
  }
</style>
