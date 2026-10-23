<script lang="ts">
  import { untrack } from 'svelte';
  import {
    deleteRcFile,
    getRcFile,
    getRcFiles,
    getTaskrcText,
    importRcFiles,
    importTaskrcUrl,
    putRcFile,
    restoreTaskrc,
  } from './api';
  import { FileText, Folder, Link, Plus, RotateCcw, Trash2, TriangleAlert, Upload } from './icons';
  import { includeLine, listing, ROOT, uploadFiles, withInclude, type Picked } from './rcpaths';
  import { applyTheme, colourOff, expandIncludes, fetchTheme, THEMES, withColour } from './themes';
  import { store } from './store.svelte';
  import type { RcFiles, TaskrcResponse } from './types';

  /** There are edits not yet applied (the tab shows a dot). */
  let { ondirty }: { ondirty?: (dirty: boolean) => void } = $props();

  /** The files of the folder, as the server lists them (`null` until it has, or if it can't). */
  let folder = $state<RcFiles | null>(null);
  /** The file shown. The address names it (`/settings/taskrc/<path>`); the main taskrc is the default. */
  let current = $state(store.settingsFile ?? ROOT);
  /** Each file opened: what is in its box, and what is saved (to detect edits and to revert to). */
  let drafts = $state<Record<string, { text: string; saved: string }>>({});
  let loading = $state(true);
  let busy = $state(false);
  let result = $state<TaskrcResponse | null>(null);
  let skipped = $state<string[]>([]);
  let error = $state<string | null>(null);
  let confirmRestore = $state(false);
  let confirmDelete = $state(false);
  let link = $state('');
  let newName = $state('');
  let replace = $state(false);
  let includeIn = $state(ROOT);

  const draft = $derived(drafts[current]);
  const text = $derived(draft?.text ?? '');
  const edited = $derived(draft ? draft.text !== draft.saved : false);
  const anyEdited = $derived(Object.values(drafts).some((d) => d.text !== d.saved));
  const paths = $derived(folder ? folder.files.map((f) => f.path) : [ROOT]);
  const rows = $derived(listing(paths.includes(ROOT) ? paths : [ROOT, ...paths]));
  const info = $derived(folder?.files.find((f) => f.path === current));
  const others = $derived(paths.filter((p) => p !== current));
  const mine = $derived(folder?.problems.filter((p) => p.file === current) ?? []);
  const isRoot = $derived(current === ROOT);

  async function fetchFile(path: string): Promise<string> {
    return path === ROOT ? getTaskrcText() : getRcFile(path);
  }

  async function refreshList() {
    try {
      folder = await getRcFiles();
    } catch {
      /* the list is a convenience: the file shown still works */
    }
  }

  // Open a file: its box starts from what is saved, so an edit changes it rather than replacing it with a blank page.
  async function open(path: string, force = false) {
    current = path;
    store.settingsFile = path === ROOT ? null : path;
    confirmDelete = false;
    if (drafts[path] && !force) return;
    loading = true;
    try {
      const saved = await fetchFile(path);
      drafts[path] = { text: saved, saved };
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  // Start with the list and the file the address names.
  $effect(() => {
    untrack(() => {
      void refreshList();
      void open(current);
    });
  });

  // The address changed (a link, the back button): show that file.
  $effect(() => {
    const want = store.settingsFile ?? ROOT;
    untrack(() => {
      if (want !== current) void open(want);
    });
  });

  // The saved settings changed somewhere else (the console's `config`, another tab of this page): follow them,
  // unless there are edits here, which are not thrown away.
  let seen = store.config;
  $effect(() => {
    const c = store.config;
    if (c === seen) return;
    seen = c;
    if (!anyEdited && !busy) {
      drafts = {};
      void open(current, true);
      void refreshList();
    }
  });

  function setText(value: string) {
    if (draft) draft.text = value;
  }

  async function onfile(e: Event) {
    const f = (e.currentTarget as HTMLInputElement).files?.[0];
    if (f) {
      setText(await f.text());
      result = null;
    }
  }

  // The server returns what it actually saved: show that, not what was pasted, so a credential that was in a pasted
  // file is gone from the page as well as from storage.
  function applied(r: TaskrcResponse, path = current) {
    result = r;
    drafts[path] = { text: r.text, saved: r.text };
  }

  // A theme: its colour lines replace the box's colour lines (Apply keeps them). Taskwarrior's are static files.
  async function pickTheme(e: Event) {
    const el = e.currentTarget as HTMLSelectElement;
    const id = el.value;
    el.value = '';
    if (!id) return;
    error = null;
    try {
      setText(applyTheme(text, id, id === 'meghnaad' ? '' : await fetchTheme(id)));
      result = null;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    }
  }

  const coloursOff = $derived(colourOff(text));

  async function run(work: () => Promise<void>) {
    busy = true;
    error = null;
    try {
      await work();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  const save = () =>
    run(async () => {
      // `include dark-256.theme`, as a desktop taskrc has it, becomes that theme's lines.
      const body = await expandIncludes(text, fetchTheme);
      applied(await putRcFile(current, body));
      skipped = [];
      await store.settingsChanged();
      await refreshList();
    });

  // The Worker fetches the link and saves what it keeps, as a paste does: the file never reaches this page.
  const fromLink = () =>
    run(async () => {
      const r = await importTaskrcUrl(link.trim());
      link = '';
      drafts = {};
      await open(ROOT, true);
      applied(r, ROOT);
      await store.settingsChanged();
      await refreshList();
    });

  const restore = () => {
    if (!confirmRestore) {
      confirmRestore = true;
      setTimeout(() => (confirmRestore = false), 4000);
      return Promise.resolve();
    }
    confirmRestore = false;
    return run(async () => {
      const r = await restoreTaskrc();
      drafts = {};
      await refreshList();
      if (folder && !folder.files.some((f) => f.path === current)) current = ROOT;
      await open(current, true);
      result = r;
      await store.settingsChanged();
    });
  };

  const addFile = () =>
    run(async () => {
      const path = newName.trim().replace(/^\.\//, '');
      if (!path) return;
      if (paths.includes(path)) {
        await open(path);
        newName = '';
        return;
      }
      applied(await putRcFile(path, ''), path);
      newName = '';
      await refreshList();
      await open(path);
    });

  const removeFile = () => {
    if (isRoot) return Promise.resolve();
    if (!confirmDelete) {
      confirmDelete = true;
      setTimeout(() => (confirmDelete = false), 4000);
      return Promise.resolve();
    }
    confirmDelete = false;
    return run(async () => {
      const gone = current;
      result = await deleteRcFile(gone);
      delete drafts[gone];
      await open(ROOT);
      await store.settingsChanged();
      await refreshList();
    });
  };

  // `include <this file>` goes into another file's box (Apply keeps it), which is then shown.
  async function include() {
    const file = current;
    const target = includeIn;
    if (!drafts[target]) await open(target);
    const d = drafts[target];
    if (!d) return;
    d.text = withInclude(d.text, target, file);
    await open(target);
  }

  // Upload files or a whole folder: one step, one version to go back to.
  async function upload(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const picked: Picked[] = await Promise.all(
      [...(input.files ?? [])].map(async (f) => ({
        name: f.name,
        relative: (f as File & { webkitRelativePath?: string }).webkitRelativePath || undefined,
        text: await f.text(),
      })),
    );
    input.value = '';
    const up = uploadFiles(picked);
    skipped = up.skipped;
    if (up.files.length === 0) {
      error = 'There was no taskrc file to add.';
      return;
    }
    await run(async () => {
      const files = await Promise.all(
        up.files.map(async (f) => ({ ...f, text: await expandIncludes(f.text, fetchTheme) })),
      );
      const r = await importRcFiles(files, replace ? 'replace' : 'merge');
      drafts = {};
      await refreshList();
      if (folder && !folder.files.some((f) => f.path === current)) current = ROOT;
      await open(current, true);
      result = r;
      await store.settingsChanged();
    });
  }

  $effect(() => {
    ondirty?.(anyEdited);
  });
</script>

<section aria-label="taskrc settings">
  <h2>taskrc</h2>
  <p class="dim">
    UDAs, custom reports, contexts, holidays and journal settings live in your <code>~/.taskrc</code>, not in your
    synced data, so this app keeps a copy: a folder of files, as written. <code>include</code> works as in Taskwarrior,
    with the path written from the file that has it.
    <strong>They are saved with your tasks and stay until you change them.</strong>
    A line this app can't use is commented out, not lost.
    <strong>Sync settings and anything that looks like a credential are removed</strong>: they're never stored, logged
    or shown.
  </p>

  {#if store.config?.config_error}
    <p class="bad" role="alert">
      <TriangleAlert size={15} /> The saved settings couldn't be read ({store.config.config_error}). Tasks still work
      with the defaults. Importing below replaces them; the unreadable copy is kept aside.
    </p>
  {/if}

  <div class="browser">
    <nav class="files" aria-label="Config files">
      <ul>
        {#each rows as r (r.path || `dir:${r.folder}`)}
          {#if r.path === ''}
            <li class="dir" style:padding-left="{r.depth * 14 + 6}px"><Folder size={14} /> {r.name}</li>
          {:else}
            <li>
              <button
                class="file-row"
                class:on={r.path === current}
                aria-current={r.path === current ? 'true' : undefined}
                style:padding-left="{r.depth * 14 + 6}px"
                onclick={() => open(r.path)}
              >
                <FileText size={14} />
                <span class="name">{r.name}</span>
                {#if drafts[r.path] && drafts[r.path].text !== drafts[r.path].saved}
                  <span class="dot" title="Not applied yet"></span><span class="sr-only"> (not applied)</span>
                {/if}
                {#if folder && !folder.files.find((f) => f.path === r.path)?.used}
                  <span class="tag" title="No include reaches this file, so its settings are not used">unused</span>
                {/if}
              </button>
            </li>
          {/if}
        {/each}
      </ul>
      <form
        class="add"
        onsubmit={(e) => {
          e.preventDefault();
          void addFile();
        }}
      >
        <label class="sr-only" for="rc-new">New file</label>
        <input id="rc-new" bind:value={newName} placeholder="holidays/en-US.rc" spellcheck="false" autocomplete="off" />
        <button class="ghost btn" disabled={busy || !newName.trim()} title="Add an empty file"
          ><Plus size={14} /> Add</button
        >
      </form>
      <div class="uploads">
        <label class="upload"
          ><Upload size={14} /> Add files
          <input type="file" multiple onchange={upload} disabled={busy} />
        </label>
        <label class="upload"
          ><Folder size={14} /> Import folder
          <input type="file" multiple webkitdirectory onchange={upload} disabled={busy} />
        </label>
        <label class="check"
          ><input type="checkbox" bind:checked={replace} /> Replace what is here (needs a <code>taskrc</code>)</label
        >
      </div>
    </nav>

    <div class="editor">
      <h3 class="mono">{current}</h3>
      {#if info && info.includes.length}
        <p class="dim small">
          Includes {#each info.includes as inc, i (i)}{i ? ', ' : ''}<code>{inc.target}</code>{#if !inc.found}
              (not found){/if}{/each}.
        </p>
      {/if}
      {#each mine as p, i (i)}<p class="warn"><TriangleAlert size={14} /> Line {p.line}: {p.message}</p>{/each}

      {#if isRoot}
        <div class="colours">
          <label
            >Colour theme
            <select onchange={pickTheme} aria-label="Colour theme">
              <option value="">choose…</option>
              {#each THEMES as t (t.id)}<option value={t.id} title={t.note}>{t.label}</option>{/each}
            </select>
          </label>
          <label class="check"
            ><input
              type="checkbox"
              checked={!coloursOff}
              onchange={(e) => setText(withColour(text, e.currentTarget.checked))}
            />
            Colours</label
          >
          <span class="dim"
            >A theme replaces the colour lines below; Apply to keep it. The Meghnaad theme is what applies when there
            are none.</span
          >
        </div>
        <label class="file">Choose a file <input type="file" onchange={onfile} /></label>
        <form
          class="link"
          onsubmit={(e) => {
            e.preventDefault();
            void fromLink();
          }}
        >
          <label for="rc-link">Or import from a link</label>
          <input
            id="rc-link"
            type="url"
            bind:value={link}
            placeholder="https://raw.githubusercontent.com/you/dotfiles/main/.taskrc"
            spellcheck="false"
            autocomplete="off"
          />
          <button disabled={busy || loading || !link.trim()}><Link size={14} /> Import</button>
          <span class="dim"
            >Fetched and saved straight away (Restore previous undoes it). GitHub and GitLab file pages work too. A
            public link; keep it free of passwords. Files it includes are not fetched: add them above.</span
          >
        </form>
      {/if}

      <label for="rc-text" class="sr-only">Text of {current}</label>
      <textarea
        id="rc-text"
        value={text}
        oninput={(e) => setText(e.currentTarget.value)}
        rows="14"
        class="mono"
        disabled={loading || !draft}
        placeholder={loading ? 'Loading…' : isRoot ? 'Nothing saved yet. Paste your taskrc here.' : 'An empty file.'}
        spellcheck="false"></textarea>

      {#if !isRoot && others.length}
        <div class="include">
          <label for="rc-include">Include this file in</label>
          <select id="rc-include" bind:value={includeIn}>
            {#each others as p (p)}<option value={p}>{p}</option>{/each}
          </select>
          <button class="ghost" onclick={include} title={includeLine(includeIn, current)}>Include</button>
          <span class="dim small">adds <code>{includeLine(includeIn, current)}</code> to that file; Apply keeps it</span
          >
        </div>
      {/if}

      {#if error}<p class="err" role="alert">{error}</p>{/if}

      {#if skipped.length}
        <p class="warn" role="status">
          <TriangleAlert size={14} /> Left out: <span class="mono">{skipped.join(', ')}</span>
        </p>
      {/if}

      {#if result}
        <div class="result" role="status">
          <p class="ok">
            Saved {result.udas} UDA{result.udas === 1 ? '' : 's'}, {result.reports} report{result.reports === 1
              ? ''
              : 's'}, {result.contexts} context{result.contexts === 1 ? '' : 's'}.
          </p>
          {#if result.blocked.length}
            <p>
              <strong>Removed for safety ({result.blocked.length}):</strong>
              <span class="mono dim">{result.blocked.join(', ')}</span>
            </p>
          {/if}
          {#if result.commented?.length}
            <details open>
              <summary class="dim"
                >{result.commented.length} line{result.commented.length === 1 ? '' : 's'} commented out</summary
              >
              {#each result.commented as c, i (i)}
                <p class="dim small"><span class="mono">{c.file ? `${c.file}:` : 'line '}{c.line}</span> {c.message}</p>
              {/each}
            </details>
          {/if}
          {#each result.warnings as w, _i (_i)}<p class="warn"><TriangleAlert size={14} /> {w}</p>{/each}
          {#if result.ignored.length}
            <details>
              <summary class="dim">{result.ignored.length} other settings not used</summary>
              <p class="mono dim">{result.ignored.join(', ')}</p>
            </details>
          {/if}
        </div>
      {/if}

      <footer class="row">
        {#if store.config?.has_previous}
          <button
            class="ghost btn"
            onclick={restore}
            disabled={busy}
            title="Go back to the files from before the last save"
          >
            <RotateCcw size={14} />
            {confirmRestore ? 'Click again to restore' : 'Restore previous'}
          </button>
        {/if}
        {#if !isRoot}
          <button class="ghost btn danger" onclick={removeFile} disabled={busy} title="Delete this file">
            <Trash2 size={14} />
            {confirmDelete ? 'Click again to delete' : 'Delete file'}
          </button>
        {/if}
        {#if edited}
          <button class="ghost" onclick={() => setText(draft?.saved ?? '')}>Revert edits</button>
        {/if}
        <span class="grow"></span>
        <button class="primary" onclick={save} disabled={busy || loading || !edited}>Apply</button>
      </footer>
    </div>
  </div>
</section>

<style>
  h2 {
    margin: 0 0 4px;
    font-size: 18px;
  }
  h3 {
    margin: 0 0 4px;
    font-size: 14px;
    overflow-wrap: anywhere;
  }
  textarea {
    width: 100%;
    margin: 8px 0;
  }
  .browser {
    display: flex;
    flex-wrap: wrap;
    gap: 12px 18px;
    align-items: flex-start;
    margin-top: 10px;
  }
  .files {
    flex: 0 0 220px;
    min-width: 0;
  }
  .editor {
    flex: 1 1 380px;
    min-width: 0;
  }
  .files ul {
    list-style: none;
    margin: 0 0 8px;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 6px;
    max-height: 40vh;
    overflow: auto;
  }
  .dir {
    display: flex;
    align-items: center;
    gap: 5px;
    padding-block: 4px;
    color: var(--dim);
    font-size: 13px;
  }
  .file-row {
    display: flex;
    align-items: center;
    gap: 5px;
    width: 100%;
    text-align: left;
    background: none;
    border: 0;
    border-radius: 0;
    padding-block: 5px;
    padding-right: 6px;
    font-size: 13px;
  }
  .file-row.on {
    background: var(--panel-2);
    font-weight: 600;
  }
  .file-row .name {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .tag {
    font-size: 11px;
    color: var(--dim);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 0 4px;
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--accent, currentColor);
  }
  .add {
    display: flex;
    gap: 6px;
    margin-bottom: 8px;
  }
  .add input {
    flex: 1;
    min-width: 0;
  }
  .uploads {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 13px;
  }
  .upload {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .upload input {
    max-width: 150px;
  }
  .include {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 8px;
    margin: 4px 0 8px;
  }
  .small {
    font-size: 12.5px;
  }
  .colours {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 16px;
    margin: 0 0 10px;
  }
  .colours select {
    margin-left: 6px;
  }
  .colours .check {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .colours .dim {
    flex-basis: 100%;
    font-size: 12.5px;
  }
  .file {
    display: block;
    margin-top: 8px;
  }
  .link {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 8px;
    margin-top: 8px;
  }
  .link input {
    flex: 1 1 260px;
    min-width: 0;
  }
  .link .dim {
    flex-basis: 100%;
    font-size: 12.5px;
  }
  .warn,
  .bad {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--warn);
  }
  .bad {
    color: var(--err);
  }
  .result p {
    margin: 4px 0;
  }
  .btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  .danger {
    color: var(--err);
  }
  code {
    font-family: var(--mono);
    background: var(--panel-2);
    border-radius: 4px;
    padding: 0 4px;
    overflow-wrap: anywhere;
  }
  @media (max-width: 760px) {
    .files {
      flex-basis: 100%;
    }
    textarea {
      min-height: 40vh;
      font-size: 14px;
    }
  }
</style>
