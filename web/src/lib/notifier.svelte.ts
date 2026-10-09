import { runCli, tzOffsetSeconds } from './api';
import { DEFAULT_SETTINGS, dueNotices, prune, type Notice, type NotifySettings } from './notify';
import { store } from './store.svelte';
import type { Row } from './types';

const SETTINGS_KEY = 'tw-notify';
const SEEN_KEY = 'tw-notified';

function read<T>(key: string, fallback: T): T {
  try {
    const v = localStorage.getItem(key);
    return v ? ({ ...fallback, ...JSON.parse(v) } as T) : fallback;
  } catch {
    return fallback;
  }
}

function write(key: string, value: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    /* private mode: preferences just won't persist */
  }
}

type Permission = NotificationPermission | 'unsupported';

/**
 * Keeps a small, fresh picture of the tasks that matter for reminders (anything with a due or wait
 * date, and whatever is running), and tells you when one needs attention. It only runs while the
 * tab is open: there is no server push.
 */
class Notifier {
  settings = $state<NotifySettings>(read(SETTINGS_KEY, DEFAULT_SETTINGS));
  permission = $state<Permission>(typeof Notification === 'undefined' ? 'unsupported' : Notification.permission);
  /** Tasks that are running right now. */
  active = $state<Row[]>([]);
  /** When `active` was fetched (epoch seconds), to extend tracked time between polls. */
  polledAt = $state(Math.floor(Date.now() / 1000));
  /** Set when the last poll failed (shown in the settings dialog, never as a popup). */
  lastError = $state<string | null>(null);

  #rows: Row[] = [];
  #seen: Record<string, number> = read<Record<string, number>>(SEEN_KEY, {});
  #started = false;

  save() {
    write(SETTINGS_KEY, this.settings);
  }

  /** Turn reminders on. Must be called from a click: browsers only ask for permission then. */
  async enable() {
    if (this.permission === 'default') this.permission = await Notification.requestPermission();
    this.settings.enabled = true;
    this.save();
    // Look at fresh data now: the snapshot from the last poll may be up to a minute old, and
    // someone who just turned reminders on expects to hear about what is already due.
    await this.poll();
  }

  disable() {
    this.settings.enabled = false;
    this.save();
  }

  async poll() {
    try {
      // Tasks with a due or wait date, plus anything running; `export` returns full rows.
      const res = await runCli({
        args: ['(', 'due.any:', 'or', 'wait.any:', 'or', '+ACTIVE', ')', 'status:pending', '_rows'],
      });
      if (res.result.kind === 'json' && Array.isArray(res.result.value)) {
        this.#rows = res.result.value as Row[];
        this.active = this.#rows.filter((r) => r.start != null);
        this.polledAt = Math.floor(Date.now() / 1000);
        this.lastError = null;
        this.check();
      } else if (res.result.kind === 'error') {
        this.lastError = res.result.message;
      }
    } catch (e) {
      this.lastError = e instanceof Error ? e.message : String(e);
    }
  }

  check() {
    if (!this.settings.enabled) return;
    const now = Math.floor(Date.now() / 1000);
    const seen = new Set(Object.keys(this.#seen));
    const notices = dueNotices(this.#rows, now, this.settings, seen, tzOffsetSeconds());
    if (notices.length === 0) return;
    for (const n of notices) {
      this.#seen[n.key] = now;
      this.show(n);
    }
    this.#seen = prune(this.#seen, now);
    write(SEEN_KEY, this.#seen);
  }

  show(n: Notice) {
    // In the page: always. With the tab in front and focused, that's enough; otherwise also use
    // the system notification so it reaches you while you're in another window.
    const focused = typeof document !== 'undefined' && document.hasFocus();
    if (focused || this.permission !== 'granted') {
      store.notify(`${n.title}: ${n.body}`);
    }
    if (!focused && this.permission === 'granted') {
      try {
        const note = new Notification(n.title, { body: n.body, tag: n.key });
        note.onclick = () => {
          window.focus();
          if (n.uuid) store.openDetail(n.uuid);
          note.close();
        };
      } catch {
        store.notify(`${n.title}: ${n.body}`); // e.g. a browser that only allows service-worker notifications
      }
    }
  }

  test() {
    this.show({
      key: `test:${Date.now()}`,
      kind: 'due',
      title: 'Test reminder',
      body: 'Reminders work while this tab is open.',
      uuid: null,
    });
  }

  /** Begin polling and checking. Safe to call once at startup. */
  start() {
    if (this.#started || typeof window === 'undefined') return;
    this.#started = true;
    void this.poll();
    setInterval(() => void this.poll(), 60_000);
    // Dates pass between polls; re-check the picture we already have much more often.
    setInterval(() => this.check(), 15_000);
    document.addEventListener('visibilitychange', () => {
      if (document.visibilityState === 'visible') void this.poll();
    });
  }
}

export const notifier = new Notifier();
