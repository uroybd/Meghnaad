<script lang="ts">
  import {
    Bell,
    BellOff,
    CalendarDays,
    ChartColumn,
    ChevronDown,
    ClipboardCheck,
    Eye,
    EyeOff,
    Folder,
    ListChecks,
    Settings,
    Tag,
    Terminal,
    TrendingDown,
  } from './icons';
  import { notifier } from './notifier.svelte';
  import { review } from './review.svelte';
  import { store } from './store.svelte';

  /** Called after something was chosen, so the drawer this sits in can close. */
  let { onnavigate }: { onnavigate: () => void } = $props();

  // On a phone the top bar's pages are listed here, and Tasks opens into the list of reports (closed to begin with: it is a
  // long list, and the pages should not sit below it). On a desktop only the list of reports shows (the pages are in the top
  // bar), always open.
  let reportsOpen = $state(false);

  const VIEWS = [
    { id: 'projects', label: 'Projects', icon: Folder },
    { id: 'tags', label: 'Tags', icon: Tag },
    { id: 'summary', label: 'Summary', icon: ChartColumn },
    { id: 'calendar', label: 'Calendar', icon: CalendarDays },
    { id: 'burndown', label: 'Burndown', icon: TrendingDown },
    { id: 'console', label: 'Console', icon: Terminal },
  ] as const;

  function pick(name: string) {
    store.report = name;
    store.view = 'tasks';
    onnavigate();
  }

  function go(view: (typeof VIEWS)[number]['id']) {
    store.view = view;
    onnavigate();
  }
</script>

<nav class="menu" aria-label="Menu">
  <button
    class="ghost item tasks"
    class:on={store.view === 'tasks'}
    aria-expanded={reportsOpen}
    aria-controls="report-list"
    onclick={() => (reportsOpen = !reportsOpen)}
  >
    <ListChecks size={18} />
    <span class="grow">Tasks</span>
    <span class="state">{store.view === 'tasks' ? store.report : ''}</span>
    <span class="chev" class:open={reportsOpen}><ChevronDown size={16} /></span>
  </button>

  <div class="reports" class:collapsed={!reportsOpen} id="report-list">
    <div class="reports-head">
      <h2>Reports</h2>
      {#if store.hasHiddenReports}
        <button
          class="ghost eye"
          aria-pressed={store.showHiddenReports}
          aria-label={store.showHiddenReports
            ? 'Hide the reports named _something'
            : 'Show the reports named _something'}
          title={store.showHiddenReports
            ? 'Hide the reports whose names start with _ (helpers for other commands)'
            : 'Show the reports whose names start with _ (helpers for other commands)'}
          onclick={() => store.setShowHiddenReports(!store.showHiddenReports)}
          >{#if store.showHiddenReports}<Eye size={14} />{:else}<EyeOff size={14} />{/if}</button
        >
      {/if}
    </div>
    <ul>
      {#each store.offeredReports as r (r.name)}
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
  </div>

  <div class="views">
    {#each VIEWS as v (v.id)}
      {@const Icon = v.icon}
      <button
        class="ghost item"
        class:on={store.view === v.id}
        aria-current={store.view === v.id ? 'page' : undefined}
        onclick={() => go(v.id)}
      >
        <Icon size={18} />
        <span class="grow">{v.label}</span>
      </button>
    {/each}
    <hr />
    <button
      class="ghost item"
      class:on={review.phase !== 'idle' && review.surface === 'card'}
      disabled={review.phase !== 'idle'}
      onclick={() => {
        void review.start({ surface: 'card' });
        onnavigate();
      }}
      title="Go through the tasks that need review: mark each reviewed, skip it, edit it, change it, finish it or delete it"
    >
      <ClipboardCheck size={18} />
      <span class="grow">Review</span>
    </button>
    <button
      class="ghost item"
      onclick={() => {
        store.notifyOpen = true;
        onnavigate();
      }}
    >
      {#if notifier.settings.enabled}<Bell size={18} />{:else}<BellOff size={18} />{/if}
      <span class="grow">Reminders</span>
      <span class="state">{notifier.settings.enabled ? 'on' : 'off'}</span>
    </button>
    <button
      class="ghost item"
      class:on={store.view === 'settings'}
      aria-current={store.view === 'settings' ? 'page' : undefined}
      onclick={() => {
        store.openSettings();
        onnavigate();
      }}
    >
      <Settings size={18} />
      <span class="grow">Settings</span>
    </button>
  </div>
  {#if store.configError}<p class="err">{store.configError}</p>{/if}
</nav>

<style>
  .reports-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin: 26px 0 8px;
  }
  h2 {
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--dim);
    margin: 0;
  }
  .eye {
    line-height: 0;
    padding: 4px 6px;
    color: var(--dim);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  li + li {
    margin-top: 2px;
  }
  .report {
    width: 100%;
    text-align: left;
    padding: 7px 12px;
  }
  .report.on,
  .item.on {
    background: var(--panel-2);
    font-weight: 600;
  }

  /* The pages and their buttons are in the top bar on a desktop. */
  .item,
  .views {
    display: none;
  }

  @media (max-width: 760px) {
    .item {
      display: flex;
      align-items: center;
      gap: 12px;
      width: 100%;
      min-height: 44px;
      padding: 10px 12px;
      text-align: left;
    }
    .views {
      display: grid;
      gap: 2px;
      margin-top: 2px;
    }
    .views hr {
      border: 0;
      border-top: 1px solid var(--line);
      margin: 8px 0;
    }
    .state {
      color: var(--dim);
      font-size: 13px;
    }
    .chev {
      display: inline-flex;
      line-height: 0;
      transition: transform 0.15s ease;
    }
    .chev.open {
      transform: rotate(180deg);
    }
    /* Reports are the content of the Tasks item: indented, with no heading of their own. */
    .reports {
      margin: 0 0 4px 14px;
      padding-left: 8px;
      border-left: 1px solid var(--line);
    }
    .reports.collapsed {
      display: none;
    }
    .reports-head {
      margin: 0 0 4px;
      justify-content: flex-end;
    }
    h2 {
      display: none;
    }
    .report {
      padding: 10px 8px;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .chev {
      transition: none;
    }
  }
</style>
