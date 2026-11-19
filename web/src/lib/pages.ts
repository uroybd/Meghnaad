// The pages and settings tabs fetched on demand (spec 0023, step 4). Tasks and the console stay in the main bundle:
// they are what most visits open, and the console's prompt is on every page.

import { lazy, loadWhenIdle, type Lazy } from './lazy.svelte';

export const PAGES = {
  projects: lazy(() => import('./ProjectsView.svelte')),
  tags: lazy(() => import('./TagsView.svelte')),
  summary: lazy(() => import('./SummaryPage.svelte')),
  calendar: lazy(() => import('./CalendarPage.svelte')),
  burndown: lazy(() => import('./BurndownPage.svelte')),
  boards: lazy(() => import('./BoardsPage.svelte')),
  settings: lazy(() => import('./SettingsPage.svelte')),
} satisfies Record<string, Lazy>;

export const TABS = {
  general: lazy(() => import('./GeneralPanel.svelte')),
  appearance: lazy(() => import('./AppearancePanel.svelte')),
  keys: lazy(() => import('./KeysPanel.svelte')),
  taskrc: lazy(() => import('./TaskrcPanel.svelte')),
  tasksh: lazy(() => import('./TaskshPanel.svelte')),
  commands: lazy(() => import('./CommandsPanel.svelte')),
  hooks: lazy(() => import('./HooksPanel.svelte')),
  urgency: lazy(() => import('./UrgencyPanel.svelte')),
} satisfies Record<string, Lazy>;

/** Parts opened now and then, not on every visit: the first-run guide, the review card, reminders, the keys' help. */
export const DIALOGS = {
  setup: lazy(() => import('./SetupGuide.svelte')),
  review: lazy(() => import('./ReviewCard.svelte')),
  notify: lazy(() => import('./NotifyDialog.svelte')),
  keys: lazy(() => import('./KeysHelp.svelte')),
  export: lazy(() => import('./ExportDialog.svelte')),
  import: lazy(() => import('./ImportView.svelte')),
} satisfies Record<string, Lazy>;

/** Start fetching a page (a link to it is pointed at or focused), when it is one fetched on demand. */
export function preloadPage(view: string | null | undefined): void {
  const page = view ? (PAGES as Record<string, Lazy>)[view] : undefined;
  if (page) void page.load();
}

/** Once the app is idle after starting: the pages, so none of them waits when opened. Tabs load with Settings. */
export function preloadPages(): void {
  loadWhenIdle([...Object.values(PAGES), ...Object.values(DIALOGS)]);
}
