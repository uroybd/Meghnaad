<script lang="ts">
  import { debounce } from './debounce';
  import {
    CalendarPlus,
    ChevronDown,
    ChevronRight,
    Download,
    FilePlus,
    FileText,
    Folder,
    FolderOpen,
    FolderPlus,
    FolderUp,
    Link,
    PanelLeftClose,
    Plus,
    Upload,
  } from './icons';
  import { newNameProblem, type TreeRow } from './rcpaths';

  /**
   * The files of the taskrc folder, as a tree, with **+ New** on top (every way to add something) and the zip at the
   * bottom. It keeps no files of its own: what to show and what each action does come from the workspace.
   */
  let {
    rows,
    current,
    here,
    taken,
    busy = false,
    dirty,
    unused,
    onopen,
    ontoggle,
    onnew,
    onupload,
    onimport,
    onholidays,
    onholidayfile,
    ondownload,
    onfold,
  }: {
    rows: TreeRow[];
    /** The file shown. */
    current: string;
    /** The folder new files, folders and uploads go in ('' is the top). */
    here: string;
    /** Every path there is, files and folders, to check a new name against. */
    taken: string[];
    busy?: boolean;
    /** The file has edits not applied yet. */
    dirty: (path: string) => boolean;
    /** No include reaches the file. */
    unused: (path: string) => boolean;
    onopen: (path: string) => void;
    /** A folder's row was pressed: open or close it, and make it the one new things go in. */
    ontoggle: (folder: string) => void;
    onnew: (kind: 'file' | 'folder', path: string) => void;
    /** Files picked to upload into `here`, or a folder, as it is. */
    onupload: (e: Event) => void;
    onimport: () => void;
    onholidays: () => void;
    onholidayfile: (e: Event) => void;
    ondownload: () => void;
    onfold: () => void;
  } = $props();

  let menuOpen = $state(false);
  /** A name being typed for a new file or folder. */
  let naming = $state<'file' | 'folder' | null>(null);
  let name = $state('');
  /** What is wrong with the name, checked once typing stops. */
  let problem = $state<string | null>(null);
  const check = debounce((n: string) => (problem = n.trim() ? newNameProblem(here, n, taken) : null), 300);

  let filesInput: HTMLInputElement | undefined = $state();
  let folderInput: HTMLInputElement | undefined = $state();
  let holidayInput: HTMLInputElement | undefined = $state();

  function start(kind: 'file' | 'folder') {
    menuOpen = false;
    naming = kind;
    name = '';
    problem = null;
  }

  function submit() {
    check.cancel();
    problem = newNameProblem(here, name, taken);
    if (problem || !naming) return;
    const n = name.trim().replace(/^\.\//, '');
    onnew(naming, here ? `${here}/${n}` : n);
    naming = null;
    name = '';
  }

  const pick = (input: HTMLInputElement | undefined) => {
    menuOpen = false;
    input?.click();
  };
  const then = (fn: () => void) => () => {
    menuOpen = false;
    fn();
  };
</script>

<svelte:window
  onclick={(e) => {
    if (menuOpen && !(e.target as Element).closest?.('.new')) menuOpen = false;
  }}
  onkeydown={(e) => {
    if (e.key === 'Escape') menuOpen = false;
  }}
/>

<nav class="tree" aria-label="Config files">
  <div class="top">
    <div class="new">
      <button
        class="new-btn"
        aria-haspopup="menu"
        aria-expanded={menuOpen}
        disabled={busy}
        onclick={() => (menuOpen = !menuOpen)}><Plus size={15} /> New</button
      >
      {#if menuOpen}
        <div class="menu" role="menu" aria-label="New">
          <button role="menuitem" onclick={() => start('file')}><FilePlus size={15} /> New file</button>
          <button role="menuitem" onclick={() => start('folder')}><FolderPlus size={15} /> New folder</button>
          <hr />
          <button role="menuitem" onclick={() => pick(filesInput)}><Upload size={15} /> Upload files</button>
          <button role="menuitem" onclick={() => pick(folderInput)}><FolderUp size={15} /> Upload a folder</button>
          <button role="menuitem" onclick={then(onimport)}><Link size={15} /> Import from a link…</button>
          <hr />
          <button role="menuitem" onclick={then(onholidays)}><CalendarPlus size={15} /> Add holidays…</button>
          <button role="menuitem" onclick={() => pick(holidayInput)}><Upload size={15} /> Upload a holiday file</button>
          <p class="dim small">
            New files and uploads go in {#if here}<code>{here}</code>{:else}the top folder{/if}.
          </p>
        </div>
      {/if}
    </div>
    <button class="ghost fold" aria-label="Hide the files" title="Hide the files" onclick={onfold}
      ><PanelLeftClose size={16} /></button
    >
  </div>

  {#if naming}
    <form
      class="naming"
      onsubmit={(e) => {
        e.preventDefault();
        submit();
      }}
    >
      <label class="sr-only" for="rc-new">{naming === 'file' ? 'New file' : 'New folder'}</label>
      <span class="dim small">{here ? `${here}/` : ''}</span>
      <input
        id="rc-new"
        bind:value={name}
        oninput={() => check(name)}
        onkeydown={(e) => {
          if (e.key === 'Escape') naming = null;
        }}
        placeholder={naming === 'file' ? 'en-US.rc' : 'folder'}
        spellcheck="false"
        autocomplete="off"
        aria-invalid={!!problem}
      />
      <button class="ghost" disabled={busy || !name.trim()}>Add</button>
      {#if problem}<p class="warn small" role="alert">{problem}</p>{/if}
    </form>
  {/if}

  <ul>
    {#each rows as r (`${r.kind}:${r.path}`)}
      <li>
        {#if r.kind === 'folder'}
          <button
            class="row dir"
            class:empty={r.empty}
            class:here={r.path === here}
            style:padding-left="{r.depth * 14 + 4}px"
            aria-expanded={r.open}
            onclick={() => ontoggle(r.path)}
          >
            {#if r.open}<ChevronDown size={14} />{:else}<ChevronRight size={14} />{/if}
            {#if r.open}<FolderOpen size={14} />{:else}<Folder size={14} />{/if}
            <span class="name">{r.name}</span>
            {#if r.empty}<span class="hint" title="A folder is kept only through the files in it">empty, not saved</span
              >{/if}
          </button>
        {:else}
          <button
            class="row file-row"
            class:on={r.path === current}
            aria-current={r.path === current ? 'true' : undefined}
            style:padding-left="{r.depth * 14 + 22}px"
            onclick={() => onopen(r.path)}
          >
            <FileText size={14} />
            <span class="name">{r.name}</span>
            {#if dirty(r.path)}
              <span class="dot" title="Not applied yet"></span><span class="sr-only"> (not applied)</span>
            {/if}
            {#if unused(r.path)}
              <span class="tag" title="No include reaches this file, so its settings are not used">unused</span>
            {/if}
          </button>
        {/if}
      </li>
    {/each}
  </ul>

  <button class="ghost zip" onclick={ondownload} disabled={busy} title="Every file, in a zip that keeps the folders"
    ><Download size={14} /> Download all (.zip)</button
  >

  <input bind:this={filesInput} class="sr-only" type="file" multiple onchange={onupload} tabindex="-1" />
  <input
    bind:this={folderInput}
    class="sr-only"
    type="file"
    multiple
    webkitdirectory
    onchange={onupload}
    tabindex="-1"
  />
  <input bind:this={holidayInput} class="sr-only" type="file" onchange={onholidayfile} tabindex="-1" />
</nav>

<style>
  .tree {
    display: flex;
    flex-direction: column;
    min-height: 0;
    height: 100%;
  }
  .top {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px;
  }
  .new {
    position: relative;
  }
  .new-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border-radius: 999px;
    padding: 6px 14px;
    font-weight: 600;
    border: 1px solid var(--accent);
  }
  .fold {
    margin-left: auto;
    line-height: 0;
  }
  .menu {
    position: absolute;
    z-index: 20;
    top: calc(100% + 4px);
    left: 0;
    width: 250px;
    display: grid;
    padding: 6px;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.25);
  }
  .menu button {
    display: flex;
    align-items: center;
    gap: 10px;
    text-align: left;
    background: none;
    border: 0;
    padding: 7px 8px;
    border-radius: 5px;
  }
  .menu button:hover,
  .menu button:focus-visible {
    background: var(--panel-2);
  }
  .menu hr {
    border: 0;
    border-top: 1px solid var(--border);
    margin: 4px 0;
    width: 100%;
  }
  .menu p {
    margin: 4px 8px 2px;
  }
  .naming {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 6px;
    padding: 0 8px 8px;
  }
  .naming input {
    flex: 1;
    min-width: 0;
  }
  .naming .warn {
    flex-basis: 100%;
    margin: 0;
    color: var(--warn);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0 0 8px;
    overflow: auto;
    flex: 1;
    min-height: 0;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 5px;
    width: 100%;
    text-align: left;
    background: none;
    border: 0;
    border-radius: 0;
    padding-block: 5px;
    padding-right: 8px;
    font-size: 13px;
  }
  .dir {
    color: var(--dim);
  }
  .dir.here {
    color: var(--fg, inherit);
  }
  .dir.empty {
    opacity: 0.6;
  }
  .hint {
    font-size: 11px;
    font-style: italic;
  }
  .file-row.on {
    background: var(--panel-2);
    font-weight: 600;
  }
  .name {
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
  .zip {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    padding: 10px 12px;
    border-top: 1px solid var(--border);
    border-radius: 0;
    font-size: 13px;
  }
  .small {
    font-size: 12px;
  }
  code {
    font-family: var(--mono);
    overflow-wrap: anywhere;
  }
</style>
