<script lang="ts">
  import { onMount } from 'svelte';
  import { offerDevices } from './deviceAsk.svelte';
  import DeviceDrawer from './DeviceDrawer.svelte';
  import { loadWhenIdle } from './lazy.svelte';
  import LazyView from './LazyView.svelte';
  import { TABS as PANELS } from './pages';
  import { store } from './store.svelte';

  type Tab = 'general' | 'appearance' | 'keys' | 'taskrc' | 'tasksh' | 'kanban' | 'commands' | 'hooks' | 'urgency';
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
    { id: 'kanban', label: 'Kanban', hint: 'Kanbans: columns of tasks, and what moving a card to one runs' },
    { id: 'commands', label: 'Commands', hint: 'Words that stand for a command line, and show under ⋯ on a task' },
    {
      id: 'hooks',
      label: 'Hooks',
      hint: 'Lua scripts that run when a command starts, adds or changes a task, and ends',
    },
    { id: 'urgency', label: 'Urgency', hint: 'Urgency coefficients and inheritance' },
  ];

  // Two groups: what is Meghnaad's own, and what is Taskwarrior's (the taskrc, urgency, hooks, tasksh). The address names
  // only the tab; the group is the one that holds it.
  type Group = 'meghnaad' | 'taskwarrior';
  const GROUPS: { id: Group; label: string; hint: string; tabs: Tab[] }[] = [
    {
      id: 'meghnaad',
      label: 'Meghnaad',
      hint: 'The app itself: where it opens, how it looks, its keys, kanban and commands',
      tabs: ['general', 'appearance', 'keys', 'kanban', 'commands'],
    },
    {
      id: 'taskwarrior',
      label: 'Taskwarrior',
      hint: 'What task itself reads: the taskrc, urgency, hooks and what tasksh needs',
      tabs: ['taskrc', 'urgency', 'hooks', 'tasksh'],
    },
  ];
  // What a device may have of its own (spec 0027): the rest shows under This device read only.
  const OWN: Tab[] = ['general', 'keys'];
  const device = $derived(store.settingsDevice);
  function openView(mine: boolean) {
    store.settingsDevice = mine;
    if (mine) void offerDevices();
  }

  const byId = Object.fromEntries(TABS.map((t) => [t.id, t])) as Record<Tab, (typeof TABS)[number]>;
  const current = $derived((store.settingsTab ?? 'general') as Tab);
  const group = $derived(GROUPS.find((g) => g.tabs.includes(current)) ?? GROUPS[0]);
  // The tab last shown in each group, so going back to a group goes back to it.
  let last = $state<Record<Group, Tab>>({ meghnaad: 'general', taskwarrior: 'taskrc' });
  $effect(() => {
    last[group.id] = current;
  });

  // Each tab keeps its own edits while another is shown, and says so with a dot; a group has one while any of its do.
  let dirty = $state<Record<Tab, boolean>>({
    general: false,
    appearance: false,
    keys: false,
    taskrc: false,
    tasksh: false,
    kanban: false,
    commands: false,
    hooks: false,
    urgency: false,
  });

  // The tabs opened so far: each is drawn from the first time it is shown, and its code fetched then.
  let opened = $state<Set<Tab>>(new Set());
  $effect(() => {
    if (!opened.has(current)) opened = new Set([...opened, current]);
  });
  // The other tabs' code, once the page is idle, so switching to one doesn't wait.
  onMount(() => loadWhenIdle(Object.values(PANELS)));

  // A tab chosen from the keyboard, a link or the code may be off the edge of a narrow bar: bring it into view.
  $effect(() => {
    document.getElementById(`settings-tab-${current}`)?.scrollIntoView?.({ inline: 'nearest', block: 'nearest' });
  });

  /** Arrow keys, Home and End over a list of tabs: the index to go to, or -1 for a key that isn't one of them. */
  function step(e: KeyboardEvent, at: number, n: number, back: string[], on: string[]): number {
    let to: number;
    if (on.includes(e.key)) to = at + 1;
    else if (back.includes(e.key)) to = at - 1;
    else if (e.key === 'Home') to = 0;
    else if (e.key === 'End') to = n - 1;
    else return -1;
    e.preventDefault();
    return (to + n) % n;
  }

  function openGroup(g: Group) {
    store.settingsTab = last[g];
  }

  function onGroupKey(e: KeyboardEvent) {
    const to = step(
      e,
      GROUPS.findIndex((g) => g.id === group.id),
      GROUPS.length,
      ['ArrowLeft'],
      ['ArrowRight'],
    );
    if (to < 0) return;
    openGroup(GROUPS[to].id);
    document.getElementById(`settings-group-${GROUPS[to].id}`)?.focus();
  }

  function onTabKey(e: KeyboardEvent) {
    const tabs = group.tabs;
    const to = step(e, tabs.indexOf(current), tabs.length, ['ArrowUp', 'ArrowLeft'], ['ArrowDown', 'ArrowRight']);
    if (to < 0) return;
    store.settingsTab = tabs[to];
    document.getElementById(`settings-tab-${tabs[to]}`)?.focus();
  }
</script>

<div class="page">
  <div class="title">
    <h1>Settings</h1>
    <div class="views" role="tablist" aria-label="Whose settings">
      <button
        role="tab"
        aria-selected={!device}
        class:on={!device}
        title="settings.json: every device's"
        onclick={() => openView(false)}>Everyone</button
      >
      <button
        role="tab"
        aria-selected={device}
        class:on={device}
        title={store.deviceId
          ? `This device's own (${store.deviceId}), over everyone's`
          : "This device's own, over everyone's"}
        onclick={() => openView(true)}>This device</button
      >
    </div>
  </div>
  <div class="groups" role="tablist" aria-label="Settings" tabindex="-1" onkeydown={onGroupKey}>
    {#each GROUPS as g (g.id)}
      <button
        role="tab"
        id="settings-group-{g.id}"
        aria-selected={group.id === g.id}
        aria-controls="settings-group-panel"
        tabindex={group.id === g.id ? 0 : -1}
        class:on={group.id === g.id}
        title={g.hint}
        onclick={() => openGroup(g.id)}
      >
        {g.label}{#if g.tabs.some((t) => dirty[t])}<span class="dot" title="Not applied yet"></span><span
            class="sr-only"
          >
            (not applied)</span
          >{/if}
      </button>
    {/each}
  </div>

  <div class="body" id="settings-group-panel" role="tabpanel" aria-labelledby="settings-group-{group.id}">
    <div
      class="tabs"
      role="tablist"
      aria-label={group.label}
      aria-orientation="vertical"
      tabindex="-1"
      onkeydown={onTabKey}
    >
      {#each group.tabs as id (id)}
        <button
          role="tab"
          id="settings-tab-{id}"
          aria-selected={current === id}
          aria-controls="settings-panel-{id}"
          tabindex={current === id ? 0 : -1}
          class:on={current === id}
          title={byId[id].hint}
          onclick={() => (store.settingsTab = id)}
        >
          {byId[id].label}{#if dirty[id]}<span class="dot" title="Not applied yet"></span><span class="sr-only">
              (not applied)</span
            >{/if}
        </button>
      {/each}
    </div>

    <!-- A tab is drawn once first opened (its code fetched then, spec 0023), and then stays, so edits survive a trip to
         another tab or group. -->
    <div class="panels">
      {#each TABS as t (t.id)}
        <div role="tabpanel" id="settings-panel-{t.id}" aria-labelledby="settings-tab-{t.id}" hidden={current !== t.id}>
          {#if device && !OWN.includes(t.id)}
            <p class="shared">
              Set under <a href="/settings/{t.id}" onclick={(e) => (e.preventDefault(), openView(false))}>Everyone</a>:
              one for every device.
            </p>
          {/if}
          <fieldset class="ro" disabled={device && !OWN.includes(t.id)}>
            {#if opened.has(t.id)}<LazyView
                of={PANELS[t.id]}
                props={{ ondirty: (v: boolean) => (dirty[t.id] = v) }}
              />{/if}
          </fieldset>
        </div>
      {/each}
    </div>
  </div>
  <p class="version">Meghnaad {__APP_VERSION__}</p>
</div>
<DeviceDrawer />

<style>
  .page {
    max-width: 1080px;
    margin: 0 auto;
  }
  h1 {
    font-size: 22px;
    margin: 0;
  }
  .title {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin: 0 0 14px;
  }
  .views {
    display: inline-flex;
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: 2px;
    gap: 2px;
  }
  .views button {
    border-color: transparent;
    background: transparent;
    padding: 5px 12px;
    border-radius: 6px;
  }
  .views button.on {
    background: var(--panel-2);
    font-weight: 600;
    box-shadow: inset 0 0 0 1px var(--accent);
  }
  fieldset.ro {
    border: none;
    margin: 0;
    padding: 0;
    min-width: 0;
  }
  fieldset.ro:disabled {
    opacity: 0.7;
  }
  .shared {
    margin: 0 0 12px;
    font-size: 13px;
    color: var(--dim);
  }
  .version {
    margin: 28px 0 0;
    color: var(--dim);
    font-size: 12px;
  }
  .groups {
    display: flex;
    gap: 2px;
    border-bottom: 1px solid var(--line);
    margin-bottom: 18px;
  }
  .groups button {
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
  .groups button.on {
    border-bottom-color: var(--accent);
    font-weight: 600;
    background: var(--panel-2);
  }
  .body {
    display: grid;
    grid-template-columns: 168px minmax(0, 1fr);
    gap: 28px;
    align-items: start;
  }
  .tabs {
    display: flex;
    flex-direction: column;
    gap: 2px;
    position: sticky;
    top: 12px;
  }
  .tabs button {
    border-color: transparent;
    border-left: 2px solid transparent;
    border-radius: 0 6px 6px 0;
    background: transparent;
    padding: 7px 12px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 7px;
    text-align: left;
    white-space: nowrap;
  }
  .tabs button:hover {
    background: var(--panel-2);
  }
  .tabs button.on {
    border-left-color: var(--accent);
    font-weight: 600;
    background: var(--panel-2);
  }
  /* A phone: the group's tabs become a row above the panel, scrolled sideways when they don't fit. */
  @media (max-width: 720px) {
    .body {
      grid-template-columns: minmax(0, 1fr);
      gap: 14px;
    }
    .tabs {
      flex-direction: row;
      overflow-x: auto;
      position: static;
    }
    .tabs button {
      flex: none;
      border-left: none;
      border-bottom: 2px solid transparent;
      border-radius: 6px 6px 0 0;
    }
    .tabs button.on {
      border-bottom-color: var(--accent);
    }
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--accent);
    flex: none;
  }
  [role='tabpanel'][hidden] {
    display: none;
  }
</style>
