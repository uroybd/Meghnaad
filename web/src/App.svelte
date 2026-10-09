<script lang="ts">
  import { onMount } from 'svelte';
  import ConsoleInput from './lib/ConsoleInput.svelte';
  import {
    Bell,
    BellOff,
    CalendarDays,
    ChartColumn,
    FileText,
    Folder,
    Tag,
    ListChecks,
    Menu,
    Plus,
    SlidersHorizontal,
    Terminal,
    TrendingDown,
    TriangleAlert,
    X,
  } from './lib/icons';
  import BurndownPage from './lib/BurndownPage.svelte';
  import CalendarPage from './lib/CalendarPage.svelte';
  import ConsoleView from './lib/ConsoleView.svelte';
  import ProjectsView from './lib/ProjectsView.svelte';
  import TagsView from './lib/TagsView.svelte';
  import DetailDrawer from './lib/DetailDrawer.svelte';
  import NotifyDialog from './lib/NotifyDialog.svelte';
  import { getSetup, type SetupStatus } from './lib/api';
  import { notifier } from './lib/notifier.svelte';
  import SetupGuide from './lib/SetupGuide.svelte';
  import TimerChip from './lib/TimerChip.svelte';
  import QuickAdd from './lib/QuickAdd.svelte';
  import SettingsDialog from './lib/SettingsDialog.svelte';
  import SummaryPage from './lib/SummaryPage.svelte';
  import { chartProps } from './lib/colors';
  import { scheme } from './lib/scheme.svelte';
  import { store } from './lib/store.svelte';
  import TaskEditor from './lib/TaskEditor.svelte';
  import TasksView from './lib/TasksView.svelte';
  import UrgencyDialog from './lib/UrgencyDialog.svelte';

  // A fresh deployment has no sign-in settings yet; say what is left instead of showing errors.
  let setup = $state<SetupStatus | null>(null);
  let started = false;

  function start() {
    setup = null;
    if (started) return;
    started = true;
    store.loadConfig();
    // Reminders can wait for the table. The Worker handles one request at a time, so a poll sent
    // first would make the first screen wait behind it. If the table never loads (another view,
    // or an error), start anyway.
    setTimeout(() => notifier.start(), 4000);
  }

  $effect(() => {
    if (!setup && store.live && !store.live.loading) notifier.start();
  });

  onMount(async () => {
    const s = await getSetup();
    if (s && !s.configured) setup = s;
    else start();
  });

  // The colours the charts use (`color.calendar.*`, `color.burndown.*`, ...), as custom properties on the page.
  let chartSet: string[] = [];
  $effect(() => {
    const props = chartProps(store.config?.colors, scheme.dark);
    const root = document.documentElement.style;
    for (const k of chartSet) root.removeProperty(k);
    for (const [k, v] of Object.entries(props)) root.setProperty(k, v);
    chartSet = Object.keys(props);
  });

  // Re-check reminders and the running task after any change.
  let lastRev = store.rev;
  $effect(() => {
    if (store.rev !== lastRev) {
      lastRev = store.rev;
      // After the table has been asked for its refresh, not alongside it.
      setTimeout(() => void notifier.poll(), 300);
    }
  });

  // On a phone the report list is a slide-over panel behind the menu button.
  let navOpen = $state(false);

  function pick(name: string) {
    store.report = name;
    store.view = 'tasks';
    navOpen = false;
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && navOpen) navOpen = false;
  }
</script>

<svelte:window {onkeydown} />

{#if setup}
  <SetupGuide status={setup} onready={start} />
{:else}
  <div class="app">
    <header class="top">
      <button
        class="ghost burger"
        aria-label="Reports and quick add"
        aria-expanded={navOpen}
        aria-controls="side"
        onclick={() => (navOpen = !navOpen)}
      >
        <Menu size={20} />
      </button>
      <span class="brand">
        <img class="logo" src="/logo-64.png" width="28" height="28" alt="" />
        <strong class="name">Meghnaad</strong>
      </span>
      <nav class="tabs" aria-label="View">
        <button class:on={store.view === 'tasks'} aria-label="Tasks" onclick={() => (store.view = 'tasks')}
          ><ListChecks size={15} /> <span class="lbl">Tasks</span></button
        >
        <button class:on={store.view === 'projects'} aria-label="Projects" onclick={() => (store.view = 'projects')}
          ><Folder size={15} /> <span class="lbl">Projects</span></button
        >
        <button class:on={store.view === 'tags'} aria-label="Tags" onclick={() => (store.view = 'tags')}
          ><Tag size={15} /> <span class="lbl">Tags</span></button
        >
        <button class:on={store.view === 'summary'} aria-label="Summary" onclick={() => (store.view = 'summary')}
          ><ChartColumn size={15} /> <span class="lbl">Summary</span></button
        >
        <button class:on={store.view === 'calendar'} aria-label="Calendar" onclick={() => (store.view = 'calendar')}
          ><CalendarDays size={15} /> <span class="lbl">Calendar</span></button
        >
        <button class:on={store.view === 'burndown'} aria-label="Burndown" onclick={() => (store.view = 'burndown')}
          ><TrendingDown size={15} /> <span class="lbl">Burndown</span></button
        >
        <button class:on={store.view === 'console'} aria-label="Console" onclick={() => (store.view = 'console')}
          ><Terminal size={15} /> <span class="lbl">Console</span></button
        >
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
        >{#if notifier.settings.enabled}<Bell size={17} />{:else}<BellOff size={17} />{/if}</button
      >
      <button
        class="ghost btn"
        aria-label="urgency settings"
        onclick={() => (store.urgencyOpen = true)}
        title="Urgency coefficients and inheritance"
        ><SlidersHorizontal size={15} /> <span class="lbl">urgency</span></button
      >
      <button
        class="ghost btn"
        aria-label="taskrc settings"
        onclick={() => (store.settingsOpen = true)}
        title="Import UDAs, reports and contexts from your taskrc"
        ><FileText size={15} /> <span class="lbl">taskrc</span></button
      >
    </header>

    {#if store.config?.config_error}
      <div class="banner" role="alert">
        <TriangleAlert size={15} />
        <span
          >Your saved taskrc settings couldn't be read, so UDAs and custom reports are off for now. Nothing was deleted.</span
        >
        <button class="ghost" onclick={() => (store.settingsOpen = true)}>Fix it</button>
      </div>
    {/if}

    {#if navOpen}<button class="scrim" aria-label="Close menu" tabindex="-1" onclick={() => (navOpen = false)}
      ></button>{/if}
    <aside class="side" class:open={navOpen} id="side" aria-label="Reports">
      <div class="side-head row">
        <strong class="grow">Meghnaad</strong>
        <button class="ghost" aria-label="Close menu" onclick={() => (navOpen = false)}><X size={18} /></button>
      </div>
      <QuickAdd />
      <h2>Reports</h2>
      <ul>
        {#each store.reports as r (r.name)}
          <li>
            <button
              class="ghost report"
              class:on={store.view === 'tasks' && store.report === r.name}
              title={r.description ?? ''}
              onclick={() => pick(r.name)}>{r.name}</button
            >
          </li>
        {/each}
      </ul>
      {#if store.configError}<p class="err">{store.configError}</p>{/if}
    </aside>

    <main class="main" class:console={store.view === 'console'}>
      {#if store.view === 'tasks'}<TasksView />{:else if store.view === 'projects'}<ProjectsView
        />{:else if store.view === 'tags'}<TagsView />{:else if store.view === 'summary'}<SummaryPage
        />{:else if store.view === 'calendar'}<CalendarPage />{:else if store.view === 'burndown'}<BurndownPage
        />{:else}<ConsoleView />{/if}
    </main>

    <button
      class="fab primary"
      aria-label="New task"
      title="New task"
      onclick={() => (store.adding = { description: '', n: (store.adding?.n ?? 0) + 1 })}
    >
      <Plus size={22} />
    </button>

    <!-- The Console has its prompt inside it; every other page shares this one. -->
    {#if store.view !== 'console'}<div class="dock"><ConsoleInput /></div>{/if}

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
  {#if store.urgencyOpen}<UrgencyDialog />{/if}
  {#if store.notifyOpen}<NotifyDialog />{/if}
{/if}

<style>
  .app {
    height: 100%;
    display: grid;
    grid-template-columns: 270px 1fr;
    grid-template-rows: auto auto 1fr auto;
    grid-template-areas: 'top top' 'banner banner' 'side main' 'dock dock';
  }
  .top {
    grid-area: top;
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 12px 24px;
    border-bottom: 1px solid var(--line);
    background: var(--panel);
  }
  .brand {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    white-space: nowrap;
    font-size: 17px;
  }
  .logo {
    border-radius: 6px;
    display: block;
  }
  .tabs {
    display: flex;
    gap: 2px;
  }
  .tabs button {
    border-color: transparent;
    background: transparent;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 7px 14px;
  }
  .btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  .top button.ghost {
    line-height: 0;
  }
  .top button.btn {
    line-height: inherit;
  }
  .tabs button.on {
    background: var(--panel-2);
    border-color: var(--line);
    font-weight: 600;
  }
  .side {
    grid-area: side;
    padding: 20px 18px;
    border-right: 1px solid var(--line);
    overflow: auto;
    background: var(--panel);
  }
  .side h2 {
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--dim);
    margin: 26px 0 8px;
  }
  .side ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .report {
    width: 100%;
    text-align: left;
    padding: 7px 12px;
  }
  .side li + li {
    margin-top: 2px;
  }
  .report.on {
    background: var(--panel-2);
    font-weight: 600;
  }
  .main {
    grid-area: main;
    overflow: auto;
    padding: 24px 36px 32px;
    min-width: 0;
  }
  /* The Console fills the area and scrolls its own output, so its prompt can sit at the bottom. */
  .main.console {
    padding: 0;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }
  .dock {
    grid-area: dock;
  }
  .banner {
    grid-area: banner;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 24px;
    background: color-mix(in srgb, var(--err) 14%, var(--panel));
    border-bottom: 1px solid var(--err);
    font-size: 14px;
  }
  .toast {
    position: fixed;
    right: 16px;
    bottom: 76px;
    background: var(--panel);
    border: 1px solid var(--ok);
    color: var(--text);
    padding: 8px 14px;
    border-radius: var(--radius);
    box-shadow: 0 4px 16px rgb(0 0 0 / 0.2);
    max-width: 28em;
    z-index: 20;
  }
  .toast.bad {
    border-color: var(--err);
  }

  /* Phone-only chrome stays out of the desktop layout. */
  .burger,
  .fab,
  .scrim,
  .side-head {
    display: none;
  }

  @media (max-width: 760px) {
    .app {
      grid-template-columns: minmax(0, 1fr);
      grid-template-areas: 'top' 'banner' 'main' 'dock';
    }
    .top {
      flex-wrap: wrap;
      gap: 4px 6px;
      padding: max(6px, env(safe-area-inset-top)) max(8px, env(safe-area-inset-right)) 6px
        max(8px, env(safe-area-inset-left));
    }
    .burger {
      display: inline-flex;
      align-items: center;
      justify-content: center;
    }
    .name {
      display: none;
    }
    .top :global(.timer) {
      order: 10;
      flex-basis: 100%;
      max-width: none;
      justify-content: space-between;
    }
    .banner {
      padding-left: max(14px, env(safe-area-inset-left));
    }

    /* The report list slides in from the left. */
    .scrim {
      display: block;
      position: fixed;
      inset: 0;
      z-index: 24;
      background: rgb(0 0 0 / 0.45);
      border: 0;
      border-radius: 0;
      padding: 0;
    }
    .side {
      position: fixed;
      z-index: 25;
      top: 0;
      bottom: 0;
      left: 0;
      width: min(310px, 86vw);
      padding: max(12px, env(safe-area-inset-top)) 12px max(12px, env(safe-area-inset-bottom))
        max(12px, env(safe-area-inset-left));
      border-right: 1px solid var(--line);
      box-shadow: 8px 0 24px rgb(0 0 0 / 0.2);
      transform: translateX(-102%);
      visibility: hidden;
      transition:
        transform 0.2s ease,
        visibility 0s linear 0.2s;
    }
    .side.open {
      transform: none;
      visibility: visible;
      transition: transform 0.2s ease;
    }
    .side-head {
      display: flex;
      margin-bottom: 10px;
    }
    .report {
      padding: 10px 8px;
    }
    /* Room under the last card so the floating + never covers its buttons. */
    .main {
      padding: 8px max(10px, env(safe-area-inset-right)) 72px max(10px, env(safe-area-inset-left));
    }

    .fab {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      position: fixed;
      z-index: 12;
      width: 52px;
      height: 52px;
      padding: 0;
      border-radius: 50%;
      right: max(16px, env(safe-area-inset-right));
      bottom: calc(96px + env(safe-area-inset-bottom));
      box-shadow: 0 4px 14px rgb(0 0 0 / 0.3);
    }
    .toast {
      left: 12px;
      right: 12px;
      bottom: calc(100px + env(safe-area-inset-bottom));
      max-width: none;
    }
  }
  /* Six tabs and the settings buttons need the room: drop the words before anything is cut off. */
  @media (max-width: 1400px) {
    .top .btn .lbl {
      display: none;
    }
  }
  @media (max-width: 1100px) {
    .tabs .lbl {
      display: none;
    }
  }
  @media (max-width: 430px) {
    .tabs .lbl,
    .top .btn .lbl {
      display: none;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .side,
    .side.open {
      transition: none;
    }
  }
</style>
