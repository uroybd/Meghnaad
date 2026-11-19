<script lang="ts">
  import { untrack } from 'svelte';
  import { importTaskrcUrl, restoreTaskrc } from './api';
  import { settingsData } from './settingsData.svelte';
  import { saveAs } from './download';
  import Drawer from './Drawer.svelte';
  import HolidaysForm from './HolidaysForm.svelte';
  import { settingsOf, withSetting, type Choice } from './holidays';
  import { PanelLeftOpen, RotateCcw, TriangleAlert } from './icons';
  import { ROOT, tree, uploadFiles, withInclude, type Picked } from './rcpaths';
  import RcAbout from './RcAbout.svelte';
  import { rcCheck } from './rcCheck.svelte';
  import RcEditorBox from './RcEditorBox.svelte';
  import { rcLayout } from './rcLayout.svelte';
  import RcFileMenu from './RcFileMenu.svelte';
  import RcImport from './RcImport.svelte';
  import RcSaved from './RcSaved.svelte';
  import RcTree from './RcTree.svelte';
  import { expandIncludes, fetchTheme } from './themes';
  import { store } from './store.svelte';
  import type { RcFiles, TaskrcResponse } from './types';
  import { zipFiles } from './zip';
  import Skeleton from './Skeleton.svelte';

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
  let link = $state('');
  let replace = $state(false);
  let note = $state<string | null>(null);
  /** A holiday file to update, when its button was pressed. */
  let updating = $state<{ path: string; text: string } | null>(null);
  /** Which drawer is open. */
  let drawer = $state<'import' | 'holidays' | null>(null);

  /** Folders made here with no file in them yet: they live only in the page, since a folder is kept through its files. */
  let empty = $state<string[]>([]);
  /** The folder new files and uploads go in: the last folder pressed, else the shown file's. */
  let here = $state('');

  let workspace: HTMLElement | undefined = $state();
  const layout = rcLayout(() => workspace);

  // The text is edited in CodeMirror (`RcEditorBox`); the drafts here stay the one place it is kept.
  let box = $state<RcEditorBox>();

  const draft = $derived(drafts[current]);
  const text = $derived(draft?.text ?? '');
  const edited = $derived(draft ? draft.text !== draft.saved : false);
  const anyEdited = $derived(Object.values(drafts).some((d) => d.text !== d.saved));
  const paths = $derived(folder ? folder.files.map((f) => f.path) : [ROOT]);
  const listed = $derived(paths.includes(ROOT) ? paths : [ROOT, ...paths]);
  const rows = $derived(
    tree(
      listed,
      empty.filter((f) => !listed.some((p) => p.startsWith(`${f}/`))),
      layout.closed,
    ),
  );
  const taken = $derived([...listed, ...empty, ...rows.filter((r) => r.kind === 'folder').map((r) => r.path)]);
  const info = $derived(folder?.files.find((f) => f.path === current));
  const others = $derived(paths.filter((p) => p !== current));
  const isRoot = $derived(current === ROOT);
  const made = $derived(draft && !edited && settingsOf(draft.saved) ? draft.saved : null);
  const calendarOn = $derived((store.config?.config?.settings?.['calendar.holidays'] ?? 'none') !== 'none');

  const check = rcCheck(
    () => {
      const files = Object.fromEntries(listed.map((p) => [p, drafts[p]?.text ?? '']));
      return drafts[current] ? files : null;
    },
    () => current,
    () => folder?.problems,
  );

  async function fetchFile(path: string): Promise<string> {
    return settingsData.file(path).get();
  }

  async function refreshList() {
    try {
      folder = await settingsData.files.get();
    } catch {
      /* the list is a convenience: the file shown still works */
    }
  }

  // Open a file: its box starts from what is saved, so an edit changes it rather than replacing it with a blank page.
  async function open(path: string, force = false) {
    box?.flush();
    current = path;
    store.settingsFile = path === ROOT ? null : path;
    here = path.includes('/') ? path.slice(0, path.lastIndexOf('/')) : '';
    if (layout.narrow) layout.fold(true);
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

  // A folder's row opens or closes it, and makes it the one new files and uploads go in.
  function toggle(f: string) {
    layout.toggle(f);
    here = f;
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

  // The saved settings changed somewhere else (the console's `config`, Task colours on Appearance, another tab): follow
  // them, unless there are edits here, which are not thrown away. A save made here is followed by the save itself.
  let seen = settingsData.rev;
  $effect(() => {
    const c = settingsData.rev;
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

  // The server returns what it actually saved: show that, not what was pasted, so a credential that was in a pasted
  // file is gone from the page as well as from storage.
  function applied(r: TaskrcResponse, path = current) {
    result = r;
    drafts[path] = { text: r.text, saved: r.text };
    settingsData.file(path).set(r.text);
  }

  async function run(work: () => Promise<void>) {
    box?.flush();
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
      applied(await settingsData.putRcFile(current, body));
      skipped = [];
      await store.settingsChanged([current]);
      await refreshList();
    });

  function revert() {
    box?.cancel();
    setText(draft?.saved ?? '');
  }

  // The Worker fetches the link and saves what it keeps, as a paste does: the file never reaches this page.
  const fromLink = () =>
    run(async () => {
      const r = await importTaskrcUrl(link.trim());
      link = '';
      drawer = null;
      drafts = {};
      await open(ROOT, true);
      applied(r, ROOT);
      await store.settingsChanged([ROOT]);
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

  // A new file is saved empty at once; a new folder is only in the page until a file goes in it.
  function addNew(kind: 'file' | 'folder', path: string) {
    if (kind === 'folder') {
      empty = [...empty, path];
      layout.reveal(path);
      here = path;
      return;
    }
    void run(async () => {
      applied(await settingsData.putRcFile(path, ''), path);
      empty = empty.filter((f) => !path.startsWith(`${f}/`));
      await refreshList();
      await open(path);
    });
  }

  // The menu asks twice first.
  const removeFile = () =>
    run(async () => {
      const gone = current;
      result = await settingsData.deleteRcFile(gone);
      delete drafts[gone];
      await open(ROOT);
      await store.settingsChanged([ROOT]);
      await refreshList();
    });

  // Holidays made by Add holidays, or a holiday file uploaded: the file is saved, and the main taskrc gets the lines
  // that use it. If the main taskrc has edits that are not applied, the lines go into its box instead of over them.
  const addHolidays = (c: Choice) =>
    run(async () => {
      note = null;
      const saved = await settingsData.putRcFile(c.path, c.text);
      applied(saved, c.path);
      if (c.include || c.calendar) {
        const root = drafts[ROOT];
        const before = root ? root.text : await settingsData.file(ROOT).get();
        let next = before;
        if (c.include) next = withInclude(next, ROOT, c.path);
        if (c.calendar) next = withSetting(next, 'calendar.holidays', 'full');
        if (next !== before && root && root.text !== root.saved) {
          root.text = next;
          note = 'The lines for the holidays are in the taskrc box; Apply there keeps them.';
        } else if (next !== before) {
          applied(await settingsData.putRcFile(ROOT, next), ROOT);
        }
      }
      result = saved;
      drawer = null;
      updating = null;
      await store.settingsChanged([c.path, ROOT]);
      await refreshList();
      await open(c.path, true);
    });

  const included = (p: string) => !!folder?.files.find((f) => f.path === ROOT)?.includes.some((i) => i.found === p);

  async function holidayFile(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const f = input.files?.[0];
    input.value = '';
    if (!f) return;
    const path = `holidays/${f.name.replace(/^\.+/, '').replace(/[/\\]/g, '-') || 'holidays.rc'}`;
    await addHolidays({ path, text: await f.text(), include: !included(path), calendar: !calendarOn });
  }

  // `include <this file>` goes into another file's box (Apply keeps it), which is then shown.
  async function include(target: string) {
    box?.flush();
    const file = current;
    if (!drafts[target]) await open(target);
    const d = drafts[target];
    if (!d) return;
    d.text = withInclude(d.text, target, file);
    await open(target);
  }

  // Upload files (into the folder new things go in) or a whole folder (as it is): one step, one version to go back to.
  async function upload(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const whole = input.hasAttribute('webkitdirectory');
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
    const into = !whole && here ? up.files.map((f) => ({ ...f, path: `${here}/${f.path}` })) : up.files;
    const mode = whole && replace ? 'replace' : 'merge';
    await run(async () => {
      const files = await Promise.all(
        into.map(async (f) => ({ ...f, text: await expandIncludes(f.text, fetchTheme) })),
      );
      const r = await settingsData.importRcFiles(files, mode);
      drafts = {};
      drawer = null;
      empty = empty.filter((f) => !files.some((x) => x.path.startsWith(`${f}/`)));
      await refreshList();
      if (folder && !folder.files.some((f) => f.path === current)) current = ROOT;
      await open(current, true);
      result = r;
      await store.settingsChanged();
    });
  }

  // A download holds what is saved, never the edits in a box that are not applied (nothing unsafe is in what is saved).
  const downloadFile = () => run(async () => saveAs(current.split('/').pop() ?? current, await fetchFile(current)));

  // The whole folder, laid out as it is: unzipped, it is a folder Upload a folder takes back.
  const downloadAll = () =>
    run(async () => {
      const list = await settingsData.files.get();
      const files = await Promise.all(list.files.map(async (f) => ({ path: f.path, text: await fetchFile(f.path) })));
      saveAs('taskrc.zip', new Blob([zipFiles(files)], { type: 'application/zip' }));
    });

  $effect(() => {
    ondirty?.(anyEdited);
  });
</script>

<section class="taskrc" aria-label="taskrc settings">
  <RcAbout />

  {#if store.config?.config_error}
    <p class="bad" role="alert">
      <TriangleAlert size={15} /> The saved settings couldn't be read ({store.config.config_error}). Tasks still work
      with the defaults. Importing replaces them; the unreadable copy is kept aside.
    </p>
  {/if}

  <div
    class="workspace"
    class:folded={layout.folded}
    class:narrow={layout.narrow}
    bind:this={workspace}
    style:height={layout.fill ? `${layout.fill}px` : null}
  >
    {#if !layout.folded}
      {#if layout.narrow}<button class="backdrop" aria-label="Hide the files" onclick={() => layout.fold(true)}
        ></button>{/if}
      <aside class="side">
        <RcTree
          {rows}
          {current}
          {here}
          {taken}
          {busy}
          dirty={(p) => !!drafts[p] && drafts[p].text !== drafts[p].saved}
          unused={(p) => p !== ROOT && !!folder && !folder.files.find((f) => f.path === p)?.used}
          onopen={(p) => void open(p)}
          ontoggle={toggle}
          onnew={addNew}
          onupload={upload}
          onimport={() => (drawer = 'import')}
          onholidays={() => (drawer = 'holidays')}
          onholidayfile={holidayFile}
          ondownload={downloadAll}
          onfold={() => layout.fold(true)}
        />
      </aside>
    {/if}

    <div class="pane">
      <div class="bar">
        {#if layout.folded}
          <button
            class="ghost icon"
            aria-label="Show the files"
            title="Show the files"
            onclick={() => layout.fold(false)}><PanelLeftOpen size={16} /></button
          >
        {/if}
        <h3 class="mono">{current}</h3>
        {#if info && info.includes.length}
          <span class="dim small"
            >includes {#each info.includes as inc, i (i)}{i ? ', ' : ''}<code>{inc.target}</code>{#if !inc.found}
                (not found){/if}{/each}</span
          >
        {/if}
        <span class="grow"></span>
        <RcFileMenu
          {current}
          {others}
          {busy}
          canDownload={!busy && !loading && !!draft}
          {made}
          ondownload={downloadFile}
          onupdate={(text) => {
            updating = { path: current, text };
            drawer = 'holidays';
          }}
          oninclude={include}
          ondelete={removeFile}
        />
      </div>

      {#if loading && !draft}<Skeleton rows={10} />{/if}
      <RcEditorBox
        bind:this={box}
        name={current}
        {text}
        hint={isRoot ? 'Nothing saved yet. Paste your taskrc here.' : 'An empty file.'}
        {loading}
        ready={!!draft}
        problems={check.mine}
        ontext={setText}
      />

      <RcSaved problems={check.mine} {error} {note} {skipped} {result} />

      <footer class="row">
        {#if edited}<button class="ghost" onclick={revert}>Revert edits</button>{/if}
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
        <span class="grow"></span>
        <button class="primary" onclick={save} disabled={busy || loading || !edited}>Apply</button>
      </footer>
    </div>
  </div>

  {#if drawer === 'import'}
    <Drawer title="Import" onclose={() => (drawer = null)}>
      <RcImport {busy} bind:link bind:replace onlink={() => void fromLink()} onupload={upload} />
    </Drawer>
  {/if}

  {#if drawer === 'holidays'}
    <Drawer
      title="Add holidays"
      onclose={() => {
        drawer = null;
        updating = null;
      }}
    >
      <HolidaysForm {busy} {calendarOn} again={updating} {included} onsave={addHolidays} />
    </Drawer>
  {/if}
</section>

<style>
  .taskrc {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .workspace {
    position: relative;
    display: grid;
    grid-template-columns: 240px minmax(0, 1fr);
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--panel);
    /* The editor takes the height of the screen below the tabs, so Apply is never scrolled away. */
    height: max(420px, calc(100dvh - 230px));
    overflow: hidden;
  }
  .workspace.folded {
    grid-template-columns: minmax(0, 1fr);
  }
  .side {
    border-right: 1px solid var(--border);
    min-height: 0;
    background: var(--panel);
  }
  .narrow .side {
    position: absolute;
    z-index: 15;
    inset: 0 auto 0 0;
    width: min(280px, 85%);
    box-shadow: 4px 0 24px rgb(0 0 0 / 0.3);
  }
  .narrow {
    grid-template-columns: minmax(0, 1fr);
  }
  .backdrop {
    position: absolute;
    z-index: 14;
    inset: 0;
    border: 0;
    border-radius: 0;
    background: rgb(0 0 0 / 0.35);
  }
  .pane {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    padding: 10px 12px;
    gap: 6px;
  }
  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 10px;
  }
  h3 {
    margin: 0;
    font-size: 14px;
    overflow-wrap: anywhere;
  }
  .icon {
    line-height: 0;
  }
  .grow {
    flex: 1;
  }
  .small {
    font-size: 12.5px;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .bad {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--err);
    margin: 2px 0;
  }
  .btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  code {
    font-family: var(--mono);
    background: var(--panel-2);
    border-radius: 4px;
    padding: 0 4px;
    overflow-wrap: anywhere;
  }
</style>
