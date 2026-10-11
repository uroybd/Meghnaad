<script lang="ts">
  import CommandsPanel from './CommandsPanel.svelte';
  import HooksPanel from './HooksPanel.svelte';
  import { store } from './store.svelte';
  import TaskrcPanel from './TaskrcPanel.svelte';
  import TaskshPanel from './TaskshPanel.svelte';
  import UrgencyPanel from './UrgencyPanel.svelte';

  type Tab = 'taskrc' | 'tasksh' | 'commands' | 'hooks' | 'urgency';
  const TABS: { id: Tab; label: string; hint: string }[] = [
    { id: 'taskrc', label: 'taskrc', hint: 'UDAs, reports, contexts, colours and the rest of your taskrc' },
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
    taskrc: false,
    tasksh: false,
    commands: false,
    hooks: false,
    urgency: false,
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

  <!-- They all stay mounted, so edits survive a trip to another tab. -->
  <div
    role="tabpanel"
    id="settings-panel-taskrc"
    aria-labelledby="settings-tab-taskrc"
    hidden={store.settingsTab !== 'taskrc'}
  >
    <TaskrcPanel ondirty={(v) => (dirty.taskrc = v)} />
  </div>
  <div
    role="tabpanel"
    id="settings-panel-tasksh"
    aria-labelledby="settings-tab-tasksh"
    hidden={store.settingsTab !== 'tasksh'}
  >
    <TaskshPanel ondirty={(v) => (dirty.tasksh = v)} />
  </div>
  <div
    role="tabpanel"
    id="settings-panel-commands"
    aria-labelledby="settings-tab-commands"
    hidden={store.settingsTab !== 'commands'}
  >
    <CommandsPanel ondirty={(v) => (dirty.commands = v)} />
  </div>
  <div
    role="tabpanel"
    id="settings-panel-hooks"
    aria-labelledby="settings-tab-hooks"
    hidden={store.settingsTab !== 'hooks'}
  >
    <HooksPanel ondirty={(v) => (dirty.hooks = v)} />
  </div>
  <div
    role="tabpanel"
    id="settings-panel-urgency"
    aria-labelledby="settings-tab-urgency"
    hidden={store.settingsTab !== 'urgency'}
  >
    <UrgencyPanel ondirty={(v) => (dirty.urgency = v)} />
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
