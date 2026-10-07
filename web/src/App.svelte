<script lang="ts">
  import { onMount } from 'svelte';
  import ConsoleInput from './lib/ConsoleInput.svelte';
  import { Bell, BellOff, FileText, ListChecks, Terminal, TriangleAlert } from './lib/icons';
  import ConsoleView from './lib/ConsoleView.svelte';
  import DetailDrawer from './lib/DetailDrawer.svelte';
  import NotifyDialog from './lib/NotifyDialog.svelte';
  import { notifier } from './lib/notifier.svelte';
  import TimerChip from './lib/TimerChip.svelte';
  import QuickAdd from './lib/QuickAdd.svelte';
  import SettingsDialog from './lib/SettingsDialog.svelte';
  import { store } from './lib/store.svelte';
  import TaskEditor from './lib/TaskEditor.svelte';
  import TasksView from './lib/TasksView.svelte';

  onMount(() => {
    store.loadConfig();
    notifier.start();
  });

  // Re-check reminders and the running task after any change.
  let lastRev = store.rev;
  $effect(() => {
    if (store.rev !== lastRev) {
      lastRev = store.rev;
      void notifier.poll();
    }
  });

  function pick(name: string) {
    store.report = name;
    store.view = 'tasks';
  }
</script>

<div class="app">
  <header class="top">
    <span class="brand">
      <img class="logo" src="/logo-64.png" width="28" height="28" alt="" />
      <strong>Meghnaad</strong>
    </span>
    <nav class="tabs" aria-label="View">
      <button class:on={store.view === 'tasks'} onclick={() => (store.view = 'tasks')}><ListChecks size={15} /> Tasks</button>
      <button class:on={store.view === 'console'} onclick={() => (store.view = 'console')}><Terminal size={15} /> Console</button>
    </nav>
    <span class="grow"></span>
    <TimerChip />
    {#if store.config?.config.active_context}
      <span class="chip" title="Active context from your taskrc">context: {store.config.config.active_context}</span>
    {/if}
    <button
      class="ghost"
      onclick={() => (store.notifyOpen = true)}
      aria-label="Reminders ({notifier.settings.enabled ? 'on' : 'off'})"
      title={notifier.settings.enabled ? 'Reminders are on (while this tab is open)' : 'Reminders are off'}
    >{#if notifier.settings.enabled}<Bell size={17} />{:else}<BellOff size={17} />{/if}</button>
    <button class="ghost btn" onclick={() => (store.settingsOpen = true)} title="Import UDAs, reports and contexts from your taskrc"><FileText size={15} /> taskrc</button>
  </header>

  {#if store.config?.config_error}
    <div class="banner" role="alert">
      <TriangleAlert size={15} />
      <span>Your saved taskrc settings couldn't be read, so UDAs and custom reports are off for now. Nothing was deleted.</span>
      <button class="ghost" onclick={() => (store.settingsOpen = true)}>Fix it</button>
    </div>
  {/if}

  <aside class="side">
    <QuickAdd />
    <h2>Reports</h2>
    <ul>
      {#each store.reports as r (r.name)}
        <li>
          <button
            class="ghost report"
            class:on={store.view === 'tasks' && store.report === r.name}
            title={r.description ?? ''}
            onclick={() => pick(r.name)}
          >{r.name}</button>
        </li>
      {/each}
    </ul>
    {#if store.configError}<p class="err">{store.configError}</p>{/if}
  </aside>

  <main class="main">
    {#if store.view === 'tasks'}<TasksView />{:else}<ConsoleView />{/if}
  </main>

  <div class="dock"><ConsoleInput /></div>

  {#if store.toast}
    <div class="toast" class:bad={store.toast.kind === 'err'} role="status">{store.toast.text}</div>
  {/if}
</div>

{#if store.editing}
  {#key store.editing.row.uuid}
    <TaskEditor row={store.editing.row} from={store.editing.from} onclose={() => (store.editing = null)} />
  {/key}
{/if}
{#if store.adding}
  {#key store.adding.n}
    <TaskEditor
      row={null}
      from={store.view === 'tasks' ? store.live : null}
      initialDescription={store.adding.description}
      onclose={() => (store.adding = null)}
      onadded={(again) => (store.adding = again ? { description: '', n: (store.adding?.n ?? 0) + 1 } : null)}
    />
  {/key}
{/if}
{#if store.detail}
  {#key store.detail.uuid}<DetailDrawer uuid={store.detail.uuid} />{/key}
{/if}
{#if store.settingsOpen}<SettingsDialog />{/if}
{#if store.notifyOpen}<NotifyDialog />{/if}

<style>
  .app {
    height: 100%;
    display: grid;
    grid-template-columns: 210px 1fr;
    grid-template-rows: auto auto 1fr auto;
    grid-template-areas: 'top top' 'banner banner' 'side main' 'dock dock';
  }
  .top { grid-area: top; display: flex; align-items: center; gap: 12px; padding: 8px 14px; border-bottom: 1px solid var(--line); background: var(--panel); }
  .brand { display: inline-flex; align-items: center; gap: 8px; white-space: nowrap; font-size: 15px; }
  .logo { border-radius: 6px; display: block; }
  .tabs { display: flex; gap: 2px; }
  .tabs button { border-color: transparent; background: transparent; display: inline-flex; align-items: center; gap: 5px; }
  .btn { display: inline-flex; align-items: center; gap: 5px; }
  .top button.ghost { line-height: 0; }
  .top button.btn { line-height: inherit; }
  .tabs button.on { background: var(--panel-2); border-color: var(--line); font-weight: 600; }
  .side { grid-area: side; padding: 12px; border-right: 1px solid var(--line); overflow: auto; background: var(--panel); }
  .side h2 { font-size: 11px; text-transform: uppercase; letter-spacing: 0.06em; color: var(--dim); margin: 16px 0 4px; }
  .side ul { list-style: none; margin: 0; padding: 0; }
  .report { width: 100%; text-align: left; padding: 3px 8px; }
  .report.on { background: var(--panel-2); font-weight: 600; }
  .main { grid-area: main; overflow: auto; padding: 12px 18px; min-width: 0; }
  .dock { grid-area: dock; }
  .banner { grid-area: banner; display: flex; align-items: center; gap: 8px; padding: 6px 14px; background: color-mix(in srgb, var(--err) 14%, var(--panel)); border-bottom: 1px solid var(--err); font-size: 13px; }
  .toast { position: fixed; right: 16px; bottom: 76px; background: var(--panel); border: 1px solid var(--ok); color: var(--text); padding: 8px 14px; border-radius: var(--radius); box-shadow: 0 4px 16px rgb(0 0 0 / 0.2); max-width: 28em; z-index: 20; }
  .toast.bad { border-color: var(--err); }

  @media (max-width: 760px) {
    .app { grid-template-columns: 1fr; grid-template-areas: 'top' 'banner' 'main' 'dock'; }
    .side { display: none; }
    .main { padding: 10px; }
  }
</style>
