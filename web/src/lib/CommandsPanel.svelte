<script lang="ts">
  import { iterativeMove, udaNames } from './kanban';
  import { untrack } from 'svelte';
  import { putAppSettings } from './api';
  import {
    MAX_SETTINGS_BYTES,
    commandLabel,
    commandProblem,
    nameProblem,
    serializeSettings,
    type TaskCommand,
  } from './appsettings';
  import IconPicker from './IconPicker.svelte';
  import { lucide } from './lucideLib.svelte';
  import { Plus, TriangleAlert, X } from './icons';
  import { store } from './store.svelte';
  import { strayAliasIcons, taskrcAliases } from './taskCommands';

  /** There are edits not yet applied (the tab shows a dot). */
  let { ondirty }: { ondirty?: (dirty: boolean) => void } = $props();

  interface Row {
    key: number;
    name: string;
    command: string;
    icon: string;
    label: string;
  }
  let next = 0;
  const toRows = (cs: TaskCommand[]): Row[] =>
    cs.map((c) => ({ key: next++, name: c.name, command: c.command, icon: c.icon ?? '', label: c.label ?? '' }));
  const toCommands = (rows: Row[]): TaskCommand[] =>
    rows.map((r) => ({
      name: r.name.trim(),
      command: r.command.trim(),
      ...(r.icon ? { icon: r.icon } : {}),
      ...(r.label.trim() ? { label: r.label.trim() } : {}),
    }));
  const same = (a: TaskCommand[], b: TaskCommand[]) => JSON.stringify(a) === JSON.stringify(b);

  const saved = $derived(store.taskCommands);
  let rows = $state<Row[]>(untrack(() => toRows(saved)));
  // What each command runs on an iterative task, where that is another line (its `status:` left out).
  const iterRuns = $derived.by(() => {
    const udas = udaNames(store.config);
    return new Map(
      rows.map((r) => [r.key, r.command.trim() ? iterativeMove(r.command, [], store.taskCommands, udas) : null]),
    );
  });
  let busy = $state(false);
  let error = $state<string | null>(null);
  let done = $state(false);

  const commands = $derived(toCommands(rows));

  // The taskrc's aliases, each with the icon it shows in the menus (kept in settings.json as `aliasIcons`).
  interface AliasRow {
    name: string;
    command: string;
    icon: string;
  }
  const aliases = $derived(taskrcAliases(store.config?.config.settings ?? {}));
  const savedIcons = $derived(store.appSettings?.aliasIcons ?? {});
  const toAliasRows = (as: typeof aliases, icons: Record<string, string>): AliasRow[] =>
    as.map((a) => ({ ...a, icon: icons[a.name] ?? '' }));
  let aliasRows = $state<AliasRow[]>(untrack(() => toAliasRows(aliases, savedIcons)));
  /** The icons to save: the saved ones (an alias the taskrc does not have keeps its icon), with the edits here. */
  const icons = $derived.by(() => {
    const out: Record<string, string> = { ...savedIcons };
    for (const a of aliasRows) {
      if (a.icon) out[a.name] = a.icon;
      else delete out[a.name];
    }
    return out;
  });
  const sameIcons = (a: Record<string, string>, b: Record<string, string>) =>
    JSON.stringify(Object.entries(a).sort()) === JSON.stringify(Object.entries(b).sort());
  const iconsEdited = $derived(!sameIcons(icons, savedIcons));
  // The aliases or their saved icons changed (the taskrc was applied, settings.json read late): follow them, unless an
  // icon was picked here.
  $effect(() => {
    const next = toAliasRows(aliases, savedIcons);
    if (!untrack(() => iconsEdited)) aliasRows = next;
  });
  /** Icons for an alias the taskrc does not have: kept in the file, and not used. */
  const stray = $derived(strayAliasIcons(savedIcons, store.config?.config.settings ?? {}));
  const commandNames = $derived(new Set(commands.map((c) => c.name)));

  const edited = $derived(!same(commands, saved) || iconsEdited);
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
      // With the taskrc, whose reports are commands too (spec 0032).
      const why = nameProblem(name, store.config?.config) ?? commandProblem(r.command);
      const dup = seen.has(name) ? `there is already a command named ${name}` : null;
      seen.add(name);
      const bad = why ?? dup ?? (r.icon && lucide.missing[r.icon] ? `no Lucide icon is called “${r.icon}”` : null);
      if (bad) out.set(r.key, bad);
    }
    return out;
  });
  /** An icon picked for an alias that Lucide does not have, by alias. */
  const aliasProblems = $derived(
    new Map(
      aliasRows.flatMap((a) =>
        a.icon && lucide.missing[a.icon] ? [[a.name, `no Lucide icon is called “${a.icon}”`] as const] : [],
      ),
    ),
  );
  const text = $derived(serializeSettings(store.appSettings?.raw ?? {}, commands, icons));
  const tooLarge = $derived(new Blob([text]).size > MAX_SETTINGS_BYTES);
  const valid = $derived(problems.size === 0 && aliasProblems.size === 0 && !tooLarge);
  /** Entries in the saved file that were not understood: they stay in the file. */
  const unread = $derived(store.appSettings?.errors ?? []);

  function add() {
    rows.push({ key: next++, name: '', command: '', icon: '', label: '' });
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
    you have selected. They are kept in this app's own <code>settings.json</code>, not in your taskrc. A name can't be
    one that is a command or report already (tasksh's <code>review</code> is one); the label, which the menus show, can say
    the same.
  </p>

  {#if unread.length}
    <div class="bad" role="alert">
      <TriangleAlert size={15} />
      <div>
        <strong>{unread.length === 1 ? 'One entry' : `${unread.length} entries`} in settings.json can't be used</strong>
        and are kept in it as they are:
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
            class="label"
            bind:value={r.label}
            oninput={() => (done = false)}
            aria-label="Label for {r.name || 'the new command'}"
            title="What the menus call it"
            placeholder={commandLabel({ name: r.name.trim() }) || 'Label'}
            autocomplete="off"
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
          {#if iterRuns.get(r.key) != null}
            <span class="iter-run">On an iterative task: <code>{iterRuns.get(r.key) || 'nothing'}</code></span>
          {/if}
        </li>
      {/each}
    </ul>
  {:else}
    <p class="dim">No commands yet.</p>
  {/if}

  <button type="button" class="btn" onclick={add}><Plus size={14} /> Add a command</button>

  <h3>Aliases</h3>
  <p class="dim">
    The aliases in your taskrc (<code>alias.&lt;name&gt;=…</code>) are in the same menus. Pick the icon each shows
    there; it is kept in <code>settings.json</code>, and the alias itself stays in the taskrc.
  </p>
  {#if stray.length}
    <div class="warn" role="status">
      <TriangleAlert size={15} />
      <div>
        <strong
          >{stray.length === 1 ? 'One icon is' : `${stray.length} icons are`} for an alias your taskrc doesn't have</strong
        >, and are kept in settings.json, unused:
        <ul>
          {#each stray as why (why)}<li>{why}</li>{/each}
        </ul>
      </div>
    </div>
  {/if}
  {#if aliasRows.length}
    <ul class="rows" aria-label="Aliases">
      {#each aliasRows as a (a.name)}
        {@const why = aliasProblems.get(a.name)}
        <li class="row-item alias" class:invalid={!!why}>
          <span class="name mono">{a.name}</span>
          <span class="cmd mono dim" title={a.command}>{a.command}</span>
          <IconPicker bind:value={a.icon} label="Icon for the alias {a.name}" />
          {#if why}<span class="why" role="alert">{why}</span>
          {:else if commandNames.has(a.name)}<span class="note dim"
              >The command {a.name} has this name and wins in the menus.</span
            >{/if}
        </li>
      {/each}
    </ul>
  {:else}
    <p class="dim">Your taskrc has no aliases.</p>
  {/if}

  {#if tooLarge}<p class="err" role="alert">
      That is more than {MAX_SETTINGS_BYTES / 1024} KB, which is the most that can be saved.
    </p>{/if}
  {#if error}<p class="err" role="alert">{error}</p>{/if}
  {#if done && !edited}<p class="ok" role="status">Applied.</p>{/if}

  <footer class="row">
    {#if edited}<button
        class="ghost"
        onclick={() => ((rows = toRows(saved)), (aliasRows = toAliasRows(aliases, savedIcons)), (done = false))}
        >Revert edits</button
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
  h3 {
    margin: 22px 0 4px;
    font-size: 15px;
  }
  .warn {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    color: var(--warn);
  }
  .warn ul {
    margin: 4px 0 0;
    padding-left: 18px;
  }
  .alias {
    grid-template-columns: minmax(90px, 160px) 1fr auto;
  }
  .alias .cmd {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .note {
    grid-column: 1 / -1;
    font-size: 12.5px;
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
    grid-template-columns: minmax(90px, 160px) minmax(90px, 180px) 1fr auto auto;
    gap: 8px;
    align-items: center;
  }
  .row-item input {
    min-width: 0;
  }
  .row-item.invalid input {
    border-color: var(--err);
  }
  /* What the command runs on an iterative task, where that is another line: muted, under the row. */
  .iter-run {
    grid-column: 1 / -1;
    color: var(--dim);
    font-size: 12.5px;
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
  footer {
    margin-top: 16px;
    gap: 8px;
  }
  @media (max-width: 760px) {
    .row-item {
      grid-template-columns: 1fr auto auto;
    }
    .row-item .name,
    .row-item .label {
      grid-column: 1 / -1;
    }
    .row-item .cmd {
      grid-column: 1;
    }
  }
</style>
