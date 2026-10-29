<script lang="ts">
  import { onMount } from 'svelte';
  import ConsoleInput from './lib/ConsoleInput.svelte';
  import { engine } from './engine/client.svelte';
  import {
    Bell,
    BellOff,
    ClipboardCheck,
    CalendarDays,
    ChartColumn,
    Folder,
    LayoutDashboard,
    Tag,
    ListChecks,
    Menu,
    Plus,
    RefreshCw,
    Settings,
    Terminal,
    TrendingDown,
    TriangleAlert,
    X,
  } from './lib/icons';
  import NavMenu from './lib/NavMenu.svelte';
  import { pullToRefresh, PULL_THRESHOLD } from './lib/pullRefresh';
  import BoardsPage from './lib/BoardsPage.svelte';
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
  import QuickAddModal from './lib/QuickAddModal.svelte';
  import SettingsPage from './lib/SettingsPage.svelte';
  import SummaryPage from './lib/SummaryPage.svelte';
  import { chartProps } from './lib/colors';
  import { parsePath, placePath, prefixFromPath } from './lib/route';
  import ReviewCard from './lib/ReviewCard.svelte';
  import { review } from './lib/review.svelte';
  import { scheme } from './lib/scheme.svelte';
  import { store } from './lib/store.svelte';
  import TaskEditor from './lib/TaskEditor.svelte';
  import TasksView from './lib/TasksView.svelte';

  // How tall the prompt is: a long command wraps, and the floating + stays above it.
  let dockHeight = $state(0);

  // A fresh deployment has no sign-in settings yet; say what is left instead of showing errors.
  let setup = $state<SetupStatus | null>(null);
  let started = false;

  // The address and the page say the same thing: every page has one (`lib/route.ts`), so a link can be shared and the back
  // button goes where it did. It is followed only once the page has read the address (a link opened cold), then both ways.
  let routed = $state(false);
  // A page that needs the reports or the boards waits for them, and for the start to have followed the address: until
  // then the page is not where the address says, and the address must not be taken for a page that has been left.
  let addressReady = $state(false);

  async function followAddress() {
    const canonical = await store.openFromAddress(location.pathname);
    // A longer prefix, or something that isn't there, is rewritten to the short address (or the front page).
    if (canonical && canonical !== location.pathname) history.replaceState(null, '', canonical);
    routed = true;
  }

  // The page moved: a new place to go back from. Not when it only found where it is (the front page, an address that was
  // nothing), when the drawer closed over the page under it, or when Settings changed tab: those take the place of the
  // address they leave.
  $effect(() => {
    if (!routed || !(addressReady || store.detail)) return;
    const want = placePath(store.place);
    const here = location.pathname;
    if (here === want) return;
    const a = parsePath(here);
    const finding = here === '/' || !a;
    const closing = a?.page === 'task' && store.place.page !== 'task';
    const tab = a?.page === 'settings' && store.place.page === 'settings';
    if (finding || closing || tab) history.replaceState(null, '', want);
    else history.pushState(null, '', want);
  });

  /** The logo and the name go to the home page. Opened in a new tab (or with a modifier key) it is just the link to `/`. */
  function goHome(e: MouseEvent) {
    if (e.metaKey || e.ctrlKey || e.shiftKey || e.altKey || e.button) return;
    e.preventDefault();
    store.goHome(true);
    navOpen = false;
  }

  function start() {
    setup = null;
    if (started) return;
    started = true;
    // The settings and `settings.json` are asked for together (they are two objects in the bucket). The settings were
    // already read when the first command started the engine, so those are used.
    void Promise.all([store.loadConfig(true), store.loadAppSettings()])
      // Where the address says wins over the home page (`/` is the home page). The reports and the boards had to be read
      // first to know what the address names, so it is followed again now that they are.
      .then(async () => {
        if (!prefixFromPath(location.pathname)) {
          const canonical = await store.openFromAddress(location.pathname);
          if (canonical && canonical !== location.pathname) history.replaceState(null, '', canonical);
        }
        addressReady = true;
      });
    void followAddress();
    // Reminders can wait for the table: the engine answers one command at a time, so a poll sent first would be
    // answered before the first screen. If the table never loads (another view, or an error), start anyway.
    setTimeout(() => notifier.start(), 4000);
  }

  $effect(() => {
    if (!setup && store.live && !store.live.loading) notifier.start();
  });

  /** This browser's copy of the tasks is thrown away (changes that never reached the server with it) and fetched again. */
  async function discardLocalCopy() {
    const sure = confirm(
      "This throws away this browser's copy of your tasks, including any change that has not reached the server yet, and loads them again from the server. Continue?",
    );
    if (!sure) return;
    await engine.forget();
    location.reload();
  }

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

  let seenSettings = $state(false);
  $effect(() => {
    if (store.view === 'settings') seenSettings = true;
  });

  // Pulling down at the top of a page, as in a browser on a phone: sync, then draw what is on screen again.
  let pulled = $state({ distance: 0, ready: false, top: 0 });
  let refreshing = $state(false);

  async function refreshPage() {
    refreshing = true;
    try {
      await engine.sync(true);
    } catch (e) {
      // The banner under the header says why too; this is the answer to the pull.
      store.notify(`Couldn't sync: ${e instanceof Error ? e.message : String(e)}`, 'err');
    }
    store.written();
    if (store.live) await store.refresh(store.live);
    refreshing = false;
  }

  const pullOptions = {
    onpull: (distance: number, ready: boolean, top: number) => (pulled = { distance, ready, top }),
    onrefresh: refreshPage,
    // A finger, not on the console (which scrolls on its own) and not while the menu is open.
    enabled: () => matchMedia('(pointer: coarse)').matches && store.view !== 'console' && !navOpen,
  };

  // On a phone the top bar's pages, and the reports under Tasks, are a slide-over menu behind the menu button.
  let navOpen = $state(false);

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && navOpen) navOpen = false;
  }
</script>

<svelte:window {onkeydown} onpopstate={() => void followAddress()} />

{#if setup}
  <SetupGuide status={setup} onready={start} />
{:else}
  <div class="app">
    <header class="top">
      <button
        class="ghost burger"
        aria-label="Menu"
        aria-expanded={navOpen}
        aria-controls="side"
        onclick={() => (navOpen = !navOpen)}
      >
        <Menu size={20} />
      </button>
      <a class="brand" href="/" aria-label="Meghnaad, home page" title="Home page" onclick={goHome}>
        <img class="logo" src="/logo-64.png" width="28" height="28" alt="" />
        <strong class="name">Meghnaad</strong>
      </a>
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
        <button class:on={store.view === 'boards'} aria-label="Boards" onclick={() => (store.view = 'boards')}
          ><LayoutDashboard size={15} /> <span class="lbl">Boards</span></button
        >
        <button class:on={store.view === 'console'} aria-label="Console" onclick={() => (store.view = 'console')}
          ><Terminal size={15} /> <span class="lbl">Console</span></button
        >
      </nav>
      <span class="grow"></span>
      <TimerChip />
      {#if store.config?.config.active_context}
        <span
          class="chip context"
          title="The active context: the console runs in it, and so does every page that follows it"
          >context: {store.config.config.active_context}</span
        >
      {/if}
      <button
        class="ghost btn review"
        class:on={review.phase !== 'idle' && review.surface === 'card'}
        aria-label="Review"
        disabled={review.phase !== 'idle'}
        onclick={() => review.start({ surface: 'card' })}
        title="Go through the tasks that need review: mark each reviewed, skip it, edit it, change it, finish it or delete it"
        ><ClipboardCheck size={15} /> <span class="lbl">Review</span></button
      >
      <button
        class="ghost bell"
        onclick={() => (store.notifyOpen = true)}
        aria-label="Reminders ({notifier.settings.enabled ? 'on' : 'off'})"
        title={notifier.settings.enabled ? 'Reminders are on (while this tab is open)' : 'Reminders are off'}
        >{#if notifier.settings.enabled}<Bell size={17} />{:else}<BellOff size={17} />{/if}</button
      >
      <button
        class="ghost btn settings"
        class:on={store.view === 'settings'}
        aria-label="Settings"
        aria-current={store.view === 'settings' ? 'page' : undefined}
        onclick={() => store.openSettings()}
        title="taskrc, tasksh and urgency settings"><Settings size={15} /> <span class="lbl">Settings</span></button
      >
    </header>

    {#if store.config?.config_error || engine.status === 'failed'}
      <div class="banners">
        {#if store.config?.config_error}
          <div class="banner" role="alert">
            <TriangleAlert size={15} />
            <span
              >Your saved taskrc settings couldn't be read, so UDAs and custom reports are off for now. Nothing was
              deleted.</span
            >
            <button class="ghost" onclick={() => store.openSettings('taskrc')}>Fix it</button>
          </div>
        {/if}
        {#if engine.status === 'failed'}
          <div class="banner" role="alert">
            <TriangleAlert size={15} />
            <span
              >Couldn't sync with the server ({engine.error}). What you change is kept in this browser and goes up when
              it works again.</span
            >
            <button class="ghost" onclick={() => void engine.sync()}>Retry</button>
            <button
              class="ghost"
              onclick={discardLocalCopy}
              title="Throw away this browser's copy and load the tasks again">Start over from the server</button
            >
          </div>
        {/if}
      </div>
    {/if}

    {#if navOpen}<button class="scrim" aria-label="Close menu" tabindex="-1" onclick={() => (navOpen = false)}
      ></button>{/if}
    <aside class="side" class:open={navOpen} id="side" aria-label="Menu">
      <div class="side-head row">
        <strong class="grow">Meghnaad</strong>
        <button class="ghost" aria-label="Close menu" onclick={() => (navOpen = false)}><X size={18} /></button>
      </div>
      <NavMenu onnavigate={() => (navOpen = false)} />
    </aside>

    <main class="main" class:console={store.view === 'console'} use:pullToRefresh={pullOptions}>
      {#if store.view === 'tasks'}<TasksView />{:else if store.view === 'projects'}<ProjectsView
        />{:else if store.view === 'tags'}<TagsView />{:else if store.view === 'summary'}<SummaryPage
        />{:else if store.view === 'calendar'}<CalendarPage />{:else if store.view === 'burndown'}<BurndownPage
        />{:else if store.view === 'boards'}<BoardsPage />{:else if store.view === 'console'}<ConsoleView />{/if}
      <!-- Once opened it stays, hidden, so what was typed in a tab is still there when you come back. -->
      {#if seenSettings}<div hidden={store.view !== 'settings'}><SettingsPage /></div>{/if}
    </main>

    {#if pulled.distance > 0 || refreshing}
      <div
        class="ptr"
        class:ready={pulled.ready}
        class:spin={refreshing}
        style="top: {pulled.top}px; --pull: {refreshing ? PULL_THRESHOLD : pulled.distance}px"
        aria-hidden="true"
      >
        <RefreshCw size={18} />
      </div>
    {/if}

    <button
      class="fab primary"
      style:bottom={dockHeight ? `calc(${dockHeight + 12}px)` : undefined}
      aria-label="New task"
      title="New task"
      onclick={() => store.quickAdd()}
    >
      <Plus size={22} />
    </button>

    <!-- The Console has its prompt inside it; every other page shares this one. -->
    {#if store.view !== 'console'}<div class="dock" bind:clientHeight={dockHeight}><ConsoleInput /></div>{/if}

    {#if store.toast}
      <div
        class="toast"
        class:bad={store.toast.kind === 'err'}
        class:busy={store.toast.kind === 'busy'}
        role="status"
        aria-busy={store.toast.kind === 'busy'}
      >
        {store.toast.text}
      </div>
    {/if}
  </div>

  {#if store.editing}
    {#key store.editing.row.uuid}
      <TaskEditor
        row={store.editing.row}
        from={store.editing.from}
        onclose={() => {
          store.editing = null;
          void review.editClosed(); // a review that opened it carries on
        }}
      />
    {/key}
  {/if}
  {#if store.adding}
    {#key store.adding.n}
      {#if store.adding.quick}
        <QuickAddModal
          line={store.adding.line}
          from={store.view === 'tasks' ? store.live : null}
          onclose={() => (store.adding = null)}
        />
      {:else}
        <TaskEditor
          row={null}
          from={store.view === 'tasks' ? store.live : null}
          seed={store.adding.seed}
          onclose={() => (store.adding = null)}
          onadded={(again) => (again ? store.addWithForm() : (store.adding = null))}
        />
      {/if}
    {/key}
  {/if}
  {#if review.surface === 'card' && review.phase !== 'idle'}<ReviewCard />{/if}
  {#if store.detail}
    {#key store.detail.uuid}<DetailDrawer uuid={store.detail.uuid} />{/key}
  {/if}
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
    color: inherit;
    text-decoration: none;
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
  .tabs button.on,
  .top button.settings.on,
  .top button.review.on {
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
  .main {
    grid-area: main;
    overflow: auto;
    padding: 24px 36px 84px;
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
  .banners {
    grid-area: banner;
  }
  .banner {
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
    bottom: 140px;
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
  .toast.busy {
    border-color: var(--border);
    color: var(--dim);
  }

  /* The round mark that follows a pull down; it sits just under the top bar. */
  .ptr {
    position: fixed;
    left: 50%;
    z-index: 15;
    width: 38px;
    height: 38px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 50%;
    background: var(--panel);
    border: 1px solid var(--line);
    box-shadow: 0 2px 10px rgb(0 0 0 / 0.2);
    color: var(--dim);
    transform: translate(-50%, calc(var(--pull) - 46px));
    opacity: min(1, calc(var(--pull) / 36px));
    pointer-events: none;
  }
  .ptr :global(svg) {
    transform: rotate(calc(var(--pull) * 4deg));
  }
  .ptr.ready {
    color: var(--accent);
  }
  .ptr.spin :global(svg) {
    animation: ptr-spin 0.8s linear infinite;
  }
  @keyframes ptr-spin {
    to {
      transform: rotate(360deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .ptr.spin :global(svg) {
      animation: none;
    }
  }

  /* The new-task button floats over every page, on a desktop as on a phone; the dock's height sets how high. */
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
    right: max(20px, env(safe-area-inset-right));
    bottom: 76px;
    box-shadow: 0 4px 14px rgb(0 0 0 / 0.3);
  }

  /* Phone-only chrome stays out of the desktop layout. */
  .burger,
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
    /* The pages and their buttons are in the drawer; the bar keeps what is live (the running task, the context). */
    .top .tabs,
    .top .review,
    .top .bell,
    .top .settings {
      display: none;
    }
    .top {
      flex-wrap: nowrap;
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
    /* Room under the last card so the floating + never covers its buttons. A pull at the top is ours (see pullRefresh.ts):
       the page does not chain it to the browser's own. */
    .main {
      overscroll-behavior-y: contain;
      padding: 8px max(10px, env(safe-area-inset-right)) 72px max(10px, env(safe-area-inset-left));
    }

    .fab {
      right: max(16px, env(safe-area-inset-right));
      bottom: calc(96px + env(safe-area-inset-bottom));
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
