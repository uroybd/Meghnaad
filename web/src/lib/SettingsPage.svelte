<script lang="ts">
  import { onMount } from 'svelte';
  import { loadWhenIdle } from './lazy.svelte';
  import LazyView from './LazyView.svelte';
  import { TABS as PANELS } from './pages';
  import { store } from './store.svelte';

  type Tab = 'general' | 'appearance' | 'keys' | 'taskrc' | 'tasksh' | 'commands' | 'hooks' | 'urgency';
  const TABS: { id: Tab; label: string; hint: string }[] = [
    { id: 'general', label: 'General', hint: 'Where the app opens' },
    { id: 'appearance', label: 'Appearance', hint: 'How the app draws things: UDAs shown as progress bars' },
    { id: 'keys', label: 'Keybindings', hint: 'The keys of the web app, the phone and the terminal app' },
    {
      id: 'taskrc',
      label: 'taskrc',
      hint: 'Your taskrc as a folder of files: UDAs, reports, contexts, holidays, colours and includes',
    },
    { id: 'tasksh', label: 'tasksh', hint: 'What tasksh review needs: the reviewed UDA and the _reviewed report' },
    { id: 'commands', label: 'Commands', hint: 'Words that stand for a command line, and show under ⋯ on a task' },
    {
      id: 'hooks',
      label: 'Hooks',
      hint: 'Lua scripts that run when a command starts, adds or changes a task, and ends',
    },
    { id: 'urgency', label: 'Urgency', hint: 'Urgency coefficients and inheritance' },
  ];

  // Each tab keeps its own edits while another is shown, and says so with a dot.
  let dirty = $state<Record<Tab, boolean>>({
    general: false,
    appearance: false,
    keys: false,
    taskrc: false,
    tasksh: false,
    commands: false,
    hooks: false,
    urgency: false,
  });

  // The tabs opened so far: each is drawn from the first time it is shown, and its code fetched then.
  let opened = $state<Set<Tab>>(new Set());
  $effect(() => {
    const tab = (store.settingsTab ?? 'general') as Tab;
    if (!opened.has(tab)) opened = new Set([...opened, tab]);
  });
  // The other tabs' code, once the page is idle, so switching to one doesn't wait.
  onMount(() => loadWhenIdle(Object.values(PANELS)));

  // A tab chosen from the keyboard, a link or the code may be off the edge of a narrow bar: bring it into view.
  $effect(() => {
    document
      .getElementById(`settings-tab-${store.settingsTab}`)
      ?.scrollIntoView?.({ inline: 'nearest', block: 'nearest' });
  });

  function onkeydown(e: KeyboardEvent) {
    const at = TABS.findIndex((t) => t.id === store.settingsTab);
    const to =
      e.key === 'ArrowRight'
        ? at + 1
        : e.key === 'ArrowLeft'
          ? at - 1
          : e.key === 'Home'
            ? 0
            : e.key === 'End'
              ? TABS.length - 1
              : -1;
    if (to < 0) return;
    e.preventDefault();
    const next = TABS[(to + TABS.length) % TABS.length].id;
    store.settingsTab = next;
    document.getElementById(`settings-tab-${next}`)?.focus();
  }
</script>

<div class="page">
  <h1>Settings</h1>
  <div class="tabs" role="tablist" aria-label="Settings" tabindex="-1" {onkeydown}>
    {#each TABS as t (t.id)}
      <button
        role="tab"
        id="settings-tab-{t.id}"
        aria-selected={store.settingsTab === t.id}
        aria-controls="settings-panel-{t.id}"
        tabindex={store.settingsTab === t.id ? 0 : -1}
        class:on={store.settingsTab === t.id}
        title={t.hint}
        onclick={() => (store.settingsTab = t.id)}
      >
        {t.label}{#if dirty[t.id]}<span class="dot" title="Not applied yet"></span><span class="sr-only">
            (not applied)</span
          >{/if}
      </button>
    {/each}
  </div>

  <!-- A tab is drawn once first opened (its code fetched then, spec 0023), and then stays, so edits survive a trip to
       another tab. -->
  <div
    role="tabpanel"
    id="settings-panel-general"
    aria-labelledby="settings-tab-general"
    hidden={store.settingsTab !== 'general'}
  >
    {#if opened.has('general')}<LazyView
        of={PANELS.general}
        props={{ ondirty: (v: boolean) => (dirty.general = v) }}
      />{/if}
  </div>
  <div
    role="tabpanel"
    id="settings-panel-appearance"
    aria-labelledby="settings-tab-appearance"
    hidden={store.settingsTab !== 'appearance'}
  >
    {#if opened.has('appearance')}<LazyView
        of={PANELS.appearance}
        props={{ ondirty: (v: boolean) => (dirty.appearance = v) }}
      />{/if}
  </div>
  <div
    role="tabpanel"
    id="settings-panel-keys"
    aria-labelledby="settings-tab-keys"
    hidden={store.settingsTab !== 'keys'}
  >
    {#if opened.has('keys')}<LazyView of={PANELS.keys} props={{ ondirty: (v: boolean) => (dirty.keys = v) }} />{/if}
  </div>
  <div
    role="tabpanel"
    id="settings-panel-taskrc"
    aria-labelledby="settings-tab-taskrc"
    hidden={store.settingsTab !== 'taskrc'}
  >
    {#if opened.has('taskrc')}<LazyView
        of={PANELS.taskrc}
        props={{ ondirty: (v: boolean) => (dirty.taskrc = v) }}
      />{/if}
  </div>
  <div
    role="tabpanel"
    id="settings-panel-tasksh"
    aria-labelledby="settings-tab-tasksh"
    hidden={store.settingsTab !== 'tasksh'}
  >
    {#if opened.has('tasksh')}<LazyView
        of={PANELS.tasksh}
        props={{ ondirty: (v: boolean) => (dirty.tasksh = v) }}
      />{/if}
  </div>
  <div
    role="tabpanel"
    id="settings-panel-commands"
    aria-labelledby="settings-tab-commands"
    hidden={store.settingsTab !== 'commands'}
  >
    {#if opened.has('commands')}<LazyView
        of={PANELS.commands}
        props={{ ondirty: (v: boolean) => (dirty.commands = v) }}
      />{/if}
  </div>
  <div
    role="tabpanel"
    id="settings-panel-hooks"
    aria-labelledby="settings-tab-hooks"
    hidden={store.settingsTab !== 'hooks'}
  >
    {#if opened.has('hooks')}<LazyView of={PANELS.hooks} props={{ ondirty: (v: boolean) => (dirty.hooks = v) }} />{/if}
  </div>
  <div
    role="tabpanel"
    id="settings-panel-urgency"
    aria-labelledby="settings-tab-urgency"
    hidden={store.settingsTab !== 'urgency'}
  >
    {#if opened.has('urgency')}<LazyView
        of={PANELS.urgency}
        props={{ ondirty: (v: boolean) => (dirty.urgency = v) }}
      />{/if}
  </div>
  <p class="version">Meghnaad {__APP_VERSION__}</p>
</div>

<style>
  .page {
    max-width: 820px;
    margin: 0 auto;
  }
  h1 {
    font-size: 22px;
    margin: 0 0 14px;
  }
  .version {
    margin: 28px 0 0;
    color: var(--dim);
    font-size: 12px;
  }
  .tabs {
    display: flex;
    gap: 2px;
    border-bottom: 1px solid var(--line);
    margin-bottom: 18px;
    overflow-x: auto;
  }
  .tabs button {
    /* Never narrower than the word: a bar too wide for the screen scrolls instead of running the names together. */
    flex: none;
    border-color: transparent;
    border-bottom: 2px solid transparent;
    border-radius: 6px 6px 0 0;
    background: transparent;
    padding: 8px 16px;
    display: inline-flex;
    align-items: center;
    gap: 7px;
    white-space: nowrap;
  }
  .tabs button.on {
    border-bottom-color: var(--accent);
    font-weight: 600;
    background: var(--panel-2);
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--accent);
  }
  [role='tabpanel'][hidden] {
    display: none;
  }
</style>
