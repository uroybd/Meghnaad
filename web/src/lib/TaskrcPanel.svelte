<script lang="ts">
  import { onDestroy, untrack } from 'svelte';
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
  import { debounce } from './debounce';
  import { saveAs } from './download';
  import Drawer from './Drawer.svelte';
  import HolidaysForm from './HolidaysForm.svelte';
  import { settingsOf, withSetting, type Choice } from './holidays';
  import {
    CalendarDays,
    Download,
    Ellipsis,
    Info,
    Link,
    PanelLeftOpen,
    RotateCcw,
    Trash2,
    TriangleAlert,
    Upload,
  } from './icons';
  import { includeLine, ROOT, tree, uploadFiles, withInclude, type Picked } from './rcpaths';
  import RcTree from './RcTree.svelte';
  import { callHelper } from '../engine/helpers';
  import { expandIncludes, fetchTheme } from './themes';
  import { store } from './store.svelte';
  import type { RcEditor } from './rcEditor';
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
  let confirmDelete = $state(false);
  let link = $state('');
  let replace = $state(false);
  let includeIn = $state(ROOT);
  let note = $state<string | null>(null);
  /** A holiday file to update, when its button was pressed. */
  let updating = $state<{ path: string; text: string } | null>(null);
  /** Which drawer is open. */
  let drawer = $state<'import' | 'holidays' | null>(null);
  /** The ⋯ menu of the file shown. */
  let moreOpen = $state(false);
  let aboutOpen = $state(false);

  // The list of files: folded away or not, and which folders are closed, remembered in this browser. On a narrow screen
  // it starts folded, and opens over the editor.
  const NARROW = '(max-width: 760px)';
  const narrow = typeof window !== 'undefined' && !!window.matchMedia?.(NARROW).matches;
  const remembered = <T,>(key: string, fallback: T): T => {
    try {
      const v = localStorage.getItem(key);
      return v === null ? fallback : (JSON.parse(v) as T);
    } catch {
      return fallback;
    }
  };
  const remember = (key: string, value: unknown) => {
    try {
      localStorage.setItem(key, JSON.stringify(value));
    } catch {
      /* private window: not remembered, and that is fine */
    }
  };
  let folded = $state(narrow || remembered('meghnaad.rc.folded', false));
  let closed = $state(new Set<string>(remembered<string[]>('meghnaad.rc.closed', [])));
  /** Folders made here with no file in them yet: they live only in the page, since a folder is kept through its files. */
  let empty = $state<string[]>([]);
  /** The folder new files and uploads go in: the last folder pressed, else the shown file's. */
  let here = $state('');

  // The workspace fills what is left of the page's scrolling area below it, so the editor takes the height and Apply
  // stays on screen, whatever sits above it (the tabs, a warning) and below it (the console bar).
  let workspace: HTMLElement | undefined = $state();
  let fill = $state<number | null>(null);
  $effect(() => {
    const ws = workspace;
    const area = ws?.closest('main') as HTMLElement | null;
    if (!ws || !area || typeof ResizeObserver === 'undefined') return;
    const measure = () => {
      const top = ws.getBoundingClientRect().top - area.getBoundingClientRect().top + area.scrollTop;
      const pad = parseFloat(getComputedStyle(area).paddingBottom) || 0;
      fill = Math.max(360, Math.floor(area.clientHeight - top - pad - 4));
    };
    measure();
    const seen = new ResizeObserver(measure);
    seen.observe(area);
    return () => seen.disconnect();
  });

  function fold(next: boolean) {
    folded = next;
    if (!narrow) remember('meghnaad.rc.folded', next);
  }

  // The text is edited in CodeMirror (line numbers, colour for what each line is, markers for lines with a problem), which loads
  // with this tab. The drafts stay the one place the text is kept: the editor shows the file shown, and what is typed
  // reaches the draft once typing stops (`typed`), read from the editor then, so nothing typed meanwhile is lost.
  let host: HTMLElement | undefined = $state();
  let editor = $state.raw<RcEditor>();
  const typed = debounce(() => {
    if (editor) setText(editor.text());
  }, 150);
  $effect(() => {
    if (!host) return;
    let alive = true;
    const parent = host;
    void import('./rcEditor').then(({ createRcEditor }) => {
      if (alive) editor = createRcEditor(parent, () => typed());
    });
    return () => {
      alive = false;
      typed.cancel();
      editor?.destroy();
      editor = undefined;
    };
  });
  $effect(() => {
    if (!editor || !draft) return;
    const hint = isRoot ? 'Nothing saved yet. Paste your taskrc here.' : 'An empty file.';
    const [name, now] = [current, text];
    untrack(() => editor?.show(name, now, hint));
  });
  $effect(() => {
    editor?.setEditable(!loading && !!draft);
  });

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
      closed,
    ),
  );
  const taken = $derived([...listed, ...empty, ...rows.filter((r) => r.kind === 'folder').map((r) => r.path)]);
  const info = $derived(folder?.files.find((f) => f.path === current));
  const others = $derived(paths.filter((p) => p !== current));
  const isRoot = $derived(current === ROOT);
  const made = $derived(draft && !edited && settingsOf(draft.saved) ? draft.saved : null);
  const calendarOn = $derived((store.config?.config?.settings?.['calendar.holidays'] ?? 'none') !== 'none');

  // What saving the files as typed would find (a line this app can't use, an include with no file, a credential),
  // worked out by the engine once typing stops, for the file shown. Until then, what the server said.
  let live = $state<{ file: string; line: number; message: string }[] | null>(null);
  const checkNow = debounce((files: Record<string, string>) => {
    try {
      live = callHelper<{ file: string; line: number; message: string }[]>('taskrc_check', files);
    } catch {
      live = null;
    }
  }, 300);
  $effect(() => {
    const files = Object.fromEntries(listed.map((p) => [p, drafts[p]?.text ?? '']));
    if (!drafts[current]) return;
    checkNow(files);
  });
  const mine = $derived(
    (live ?? folder?.problems ?? [])
      .filter((p) => p.file === current)
      .map((p) => ({ line: p.line, message: p.message })),
  );
  // The markers in the editor follow the problems; drawn again whenever another file is shown.
  $effect(() => {
    void current;
    const ed = editor;
    const problems = mine;
    untrack(() => ed?.mark(problems));
  });
  onDestroy(() => checkNow.cancel());

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
    typed.flush();
    current = path;
    store.settingsFile = path === ROOT ? null : path;
    confirmDelete = false;
    moreOpen = false;
    here = path.includes('/') ? path.slice(0, path.lastIndexOf('/')) : '';
    if (narrow) folded = true;
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
    const next = new Set(closed);
    if (!next.delete(f)) next.add(f);
    closed = next;
    here = f;
    remember('meghnaad.rc.closed', [...next]);
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

  // The saved settings changed somewhere else (the console's `config`, Task colours on Appearance): follow them,
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

  // The server returns what it actually saved: show that, not what was pasted, so a credential that was in a pasted
  // file is gone from the page as well as from storage.
  function applied(r: TaskrcResponse, path = current) {
    result = r;
    drafts[path] = { text: r.text, saved: r.text };
  }

  async function run(work: () => Promise<void>) {
    typed.flush();
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

  function revert() {
    typed.cancel();
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

  // A new file is saved empty at once; a new folder is only in the page until a file goes in it.
  function addNew(kind: 'file' | 'folder', path: string) {
    if (kind === 'folder') {
      empty = [...empty, path];
      closed = new Set([...closed].filter((f) => f !== path));
      here = path;
      return;
    }
    void run(async () => {
      applied(await putRcFile(path, ''), path);
      empty = empty.filter((f) => !path.startsWith(`${f}/`));
      await refreshList();
      await open(path);
    });
  }

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

  // Holidays made by Add holidays, or a holiday file uploaded: the file is saved, and the main taskrc gets the lines
  // that use it. If the main taskrc has edits that are not applied, the lines go into its box instead of over them.
  const addHolidays = (c: Choice) =>
    run(async () => {
      note = null;
      const saved = await putRcFile(c.path, c.text);
      applied(saved, c.path);
      if (c.include || c.calendar) {
        const root = drafts[ROOT];
        const before = root ? root.text : await getTaskrcText();
        let next = before;
        if (c.include) next = withInclude(next, ROOT, c.path);
        if (c.calendar) next = withSetting(next, 'calendar.holidays', 'full');
        if (next !== before && root && root.text !== root.saved) {
          root.text = next;
          note = 'The lines for the holidays are in the taskrc box; Apply there keeps them.';
        } else if (next !== before) {
          applied(await putRcFile(ROOT, next), ROOT);
        }
      }
      result = saved;
      drawer = null;
      updating = null;
      await store.settingsChanged();
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
  async function include() {
    typed.flush();
    const file = current;
    const target = includeIn;
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
      const r = await importRcFiles(files, mode);
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
      const list = await getRcFiles();
      const files = await Promise.all(list.files.map(async (f) => ({ path: f.path, text: await fetchFile(f.path) })));
      saveAs('taskrc.zip', new Blob([zipFiles(files)], { type: 'application/zip' }));
    });

  $effect(() => {
    ondirty?.(anyEdited);
  });
</script>

<svelte:window
  onclick={(e) => {
    if (moreOpen && !(e.target as Element).closest?.('.more')) moreOpen = false;
  }}
/>

<section class="taskrc" aria-label="taskrc settings">
  <header class="head">
    <h2>taskrc</h2>
    <span class="dim">A copy of your taskrc, as a folder of files, saved with your tasks.</span>
    <button
      class="ghost about"
      aria-expanded={aboutOpen}
      aria-label="About the taskrc copy"
      title="About the taskrc copy"
      onclick={() => (aboutOpen = !aboutOpen)}><Info size={15} /></button
    >
  </header>
  {#if aboutOpen}
    <p class="dim about-text">
      UDAs, custom reports, contexts, holidays and journal settings live in your <code>~/.taskrc</code>, not in your
      synced data, so this app keeps a copy: a folder of files, as written. <code>include</code> works as in
      Taskwarrior, with the path written from the file that has it. They are saved with your tasks and stay until you
      change them. A line this app can't use is commented out, not lost.
      <strong>Sync settings and anything that looks like a credential are removed</strong>: they're never stored, logged
      or shown. The colours are set in Appearance.
    </p>
  {/if}

  {#if store.config?.config_error}
    <p class="bad" role="alert">
      <TriangleAlert size={15} /> The saved settings couldn't be read ({store.config.config_error}). Tasks still work
      with the defaults. Importing replaces them; the unreadable copy is kept aside.
    </p>
  {/if}

  <div class="workspace" class:folded class:narrow bind:this={workspace} style:height={fill ? `${fill}px` : null}>
    {#if !folded}
      {#if narrow}<button class="backdrop" aria-label="Hide the files" onclick={() => fold(true)}></button>{/if}
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
          onfold={() => fold(true)}
        />
      </aside>
    {/if}

    <div class="pane">
      <div class="bar">
        {#if folded}
          <button class="ghost icon" aria-label="Show the files" title="Show the files" onclick={() => fold(false)}
            ><PanelLeftOpen size={16} /></button
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
        <div class="more">
          <button
            class="ghost icon"
            aria-label="More for this file"
            title="More for this file"
            aria-haspopup="menu"
            aria-expanded={moreOpen}
            onclick={() => (moreOpen = !moreOpen)}><Ellipsis size={16} /></button
          >
          {#if moreOpen}
            <div class="menu" role="menu" aria-label="This file">
              <button
                role="menuitem"
                onclick={downloadFile}
                disabled={busy || loading || !draft}
                title="Download the saved file"><Download size={14} /> Download</button
              >
              {#if made}
                <button
                  role="menuitem"
                  onclick={() => {
                    updating = { path: current, text: made };
                    drawer = 'holidays';
                    moreOpen = false;
                  }}
                  title="Fetch these holidays again, for the current years"
                  ><CalendarDays size={14} /> Update holidays</button
                >
              {/if}
              {#if !isRoot && others.length}
                <div class="include">
                  <label for="rc-include">Include this file in</label>
                  <select id="rc-include" bind:value={includeIn}>
                    {#each others as p (p)}<option value={p}>{p}</option>{/each}
                  </select>
                  <button class="ghost" onclick={include} title={includeLine(includeIn, current)}>Include</button>
                  <span class="dim small"
                    >adds <code>{includeLine(includeIn, current)}</code> to that file; Apply keeps it</span
                  >
                </div>
              {/if}
              {#if !isRoot}
                <button role="menuitem" class="danger" onclick={removeFile} disabled={busy} title="Delete this file">
                  <Trash2 size={14} />
                  {confirmDelete ? 'Click again to delete' : 'Delete file'}
                </button>
              {/if}
            </div>
          {/if}
        </div>
      </div>

      {#if loading && !draft}<Skeleton rows={10} />{/if}
      <div class="rc-editor" bind:this={host} aria-busy={loading} hidden={loading && !draft}></div>

      <div class="below">
        {#each mine as p, i (i)}<p class="warn"><TriangleAlert size={14} /> Line {p.line}: {p.message}</p>{/each}
        {#if error}<p class="err" role="alert">{error}</p>{/if}
        {#if note}<p class="dim" role="status">{note}</p>{/if}
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
                  <p class="dim small">
                    <span class="mono">{c.file ? `${c.file}:` : 'line '}{c.line}</span>
                    {c.message}
                  </p>
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
      </div>

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
      <form
        class="link"
        onsubmit={(e) => {
          e.preventDefault();
          void fromLink();
        }}
      >
        <label for="rc-link"><Link size={14} /> From a link</label>
        <input
          id="rc-link"
          type="url"
          bind:value={link}
          placeholder="https://raw.githubusercontent.com/you/dotfiles/main/.taskrc"
          spellcheck="false"
          autocomplete="off"
        />
        <button disabled={busy || !link.trim()}>Import</button>
        <p class="dim small">
          Fetched and saved as the main taskrc straight away (Restore previous undoes it). GitHub and GitLab file pages
          work too. A public link; keep it free of passwords. Files it includes are not fetched: add them with + New.
        </p>
      </form>
      <hr />
      <div class="folder-import">
        <p><Upload size={14} /> A folder, as it is</p>
        <label class="check"
          ><input type="checkbox" bind:checked={replace} /> Replace what is here (needs a <code>taskrc</code>)</label
        >
        <input type="file" multiple webkitdirectory onchange={upload} disabled={busy} aria-label="Folder to import" />
        <p class="dim small">Without Replace, the files are added to what is here, and same names are overwritten.</p>
      </div>
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
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 4px 10px;
  }
  h2 {
    margin: 0;
    font-size: 18px;
  }
  .about {
    line-height: 0;
    align-self: center;
  }
  .about-text {
    margin: 0;
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
  .rc-editor {
    flex: 1;
    min-height: 120px;
    overflow: hidden;
    border: 1px solid var(--border);
    border-radius: 6px;
  }
  .rc-editor :global(.cm-editor) {
    height: 100%;
  }
  .rc-editor :global(.cm-scroller) {
    overflow: auto;
  }
  .below {
    max-height: 30%;
    overflow: auto;
  }
  .below:empty {
    display: none;
  }
  .more {
    position: relative;
  }
  .menu {
    position: absolute;
    z-index: 20;
    right: 0;
    top: calc(100% + 4px);
    width: 280px;
    display: grid;
    gap: 2px;
    padding: 6px;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.25);
  }
  .menu > button {
    display: flex;
    align-items: center;
    gap: 8px;
    text-align: left;
    background: none;
    border: 0;
    padding: 7px 8px;
    border-radius: 5px;
  }
  .menu > button:hover {
    background: var(--panel-2);
  }
  .include {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 6px;
    padding: 6px 8px;
    border-block: 1px solid var(--border);
  }
  .include select {
    flex: 1;
    min-width: 0;
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
  .link {
    display: grid;
    gap: 8px;
  }
  .link label,
  .folder-import p:first-child {
    display: flex;
    align-items: center;
    gap: 6px;
    font-weight: 600;
    margin: 0;
  }
  .folder-import {
    display: grid;
    gap: 8px;
  }
  hr {
    border: 0;
    border-top: 1px solid var(--border);
    margin: 16px 0;
  }
  .check {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .warn,
  .bad {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--warn);
    margin: 2px 0;
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
</style>
