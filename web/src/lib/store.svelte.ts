import { engine } from '../engine/client.svelte';
import { refreshHooks } from './api';
import { settingsData } from './settingsData.svelte';
import { TasksStore } from './store/tasks.svelte';

export type { Entry, Toast } from './store/base.svelte';
export { HISTORY_MAX, shortenUuids } from './store/console.svelte';

/**
 * The one store every component imports. It is built in parts, each in `store/` and each on the one before (spec 0023):
 * `base` (where the app is, the toast, the names offered for completion), `settings` (the config, `settings.json`, the
 * keys, the contexts), `navigation` (the pages, their filters, the drawers, the address), `console` (its entries, the
 * history, running a line) and `tasks` (the live table, the pages' panels, questions and the buttons' actions). This
 * adds what follows the engine: the clock, and changes that arrive from elsewhere.
 */
class Store extends TasksStore {
  constructor() {
    super();
    // Keep relative dates ("due in 2d") fresh without re-fetching.
    if (typeof window !== 'undefined') {
      setInterval(() => (this.now = Math.floor(Date.now() / 1000)), 30_000);
      // What another device or tab did arrives with a sync; draw again what is on screen.
      engine.onChanged(() => {
        this.written();
        if (this.live) void this.refresh(this.live);
      });
      // The static site starts from the settings this device kept; the branch's, read after, differed.
      engine.onSettingsChanged((path) => {
        if (path === '/api/config') void this.settingsChanged();
        else if (path === '/api/settings') void this.loadAppSettings();
        else {
          settingsData.hooksChanged();
          void refreshHooks();
        }
      });
    }
  }
}

export const store = new Store();
