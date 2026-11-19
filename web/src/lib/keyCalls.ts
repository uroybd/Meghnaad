// What a key of the keymap, or a `:` line, does in the web app (spec 0020). The engine says what was asked
// (`tc_core::keymap`: an action, the words after it and a count); this does it here. The Tasks table's chosen row is
// the row that has the focus: the motions move it, and the task actions act on it (or on the ticked rows).

import { tick } from 'svelte';
import { splitWords } from './cmdline';
import { review } from './review.svelte';
import { selection } from './selection.svelte';
import { store } from './store.svelte';
import type { Call } from './keymap';
import type { Row } from './types';

export const PAGES = ['tasks', 'projects', 'tags', 'summary', 'calendar', 'burndown', 'boards', 'console'] as const;
/** The pages in the order of the top bar, Settings last, as the terminal app numbers them. */
const ORDER = [...PAGES, 'settings'];

/** Focus a field of the page (and open a list where the browser can), after the page it is on is drawn. */
async function focusField(selector: string) {
  await tick();
  const el = document.querySelector<HTMLElement>(selector);
  el?.focus();
  if (el instanceof HTMLSelectElement) {
    try {
      el.showPicker();
    } catch {
      // Not every browser opens a list from code; it has the focus, so the arrow keys choose.
    }
  }
}

function openPage(page: string) {
  if (page === 'settings') store.openSettings();
  else if (PAGES.includes(page as (typeof PAGES)[number])) store.view = page as (typeof PAGES)[number];
}

/** A page by its number (1 to 9) or the start of its name, any case (`:tabn 3`, `:tabn tasks`). */
export function pageNamed(word: string): string | null {
  const n = /^\d+$/.test(word) ? Number(word) : NaN;
  if (!Number.isNaN(n)) return ORDER[n - 1] ?? null;
  const w = word.toLowerCase();
  return (w && ORDER.find((p) => p.startsWith(w))) || null;
}

/** The Tasks table's rows as drawn, and the one with the focus. */
function tableRows(): { rows: HTMLElement[]; at: number } {
  const rows = [...document.querySelectorAll<HTMLElement>('main tr[data-uuid]')];
  const focused = (document.activeElement as HTMLElement | null)?.closest?.('tr[data-uuid]');
  return { rows, at: focused ? rows.indexOf(focused as HTMLElement) : -1 };
}

function choose(rows: HTMLElement[], i: number) {
  const row = rows[Math.max(0, Math.min(rows.length - 1, i))];
  row?.focus();
  row?.scrollIntoView?.({ block: 'nearest' });
}

/** The chosen row of the Tasks table: the one with the focus. */
function chosen(): Row | null {
  const { rows, at } = tableRows();
  const uuid = rows[at]?.dataset.uuid;
  const r = store.live?.result;
  return (uuid && r?.kind === 'report' && r.rows.find((row) => row.uuid === uuid)) || null;
}

const say = (text: string) => store.notify(text, 'err');

/** The ticked tasks, or else the chosen one. */
function targets(): string[] {
  if (selection.count) return [...selection.uuids];
  const row = chosen();
  return row ? [row.uuid] : [];
}

async function onTasks(words: string[], uuids = targets()): Promise<void> {
  if (!uuids.length) {
    say('Choose a task first: j and k move through the table.');
    return;
  }
  const res = await store.act(store.live, [...uuids, ...words], { approved: uuids, confirmed: true });
  if (res?.wrote && selection.count) selection.clear();
}

/** Open the first link in a task (its description, then its annotations) in a new tab. */
function openLink(row: Row) {
  const texts = [row.description, ...(row.annotations ?? []).map((a) => a.text)];
  const link = texts
    .flatMap((t) => t.split(/\s+/))
    .find((w) => /^https?:\/\//.test(w))
    ?.replace(/[.,)\]>]+$/, '');
  if (link) window.open(link, '_blank', 'noopener');
  else say('This task has no link.');
}

/** Do what `call` asks; false for an action the web app has nothing for. */
export function runCall(call: Call): boolean {
  const args = call.args ?? [];
  const count = call.count ?? 1;
  const moving = call.action.startsWith('move.') || call.action === 'tasks.mark';
  if (moving && store.view !== 'tasks') return false;
  switch (call.action) {
    case 'move.down':
    case 'move.up': {
      const { rows, at } = tableRows();
      choose(rows, at < 0 ? 0 : at + (call.action === 'move.down' ? count : -count));
      return true;
    }
    case 'move.first':
    case 'move.last': {
      const { rows } = tableRows();
      choose(rows, call.count ? call.count - 1 : call.action === 'move.first' ? 0 : rows.length - 1);
      return true;
    }
    case 'move.row': {
      const { rows } = tableRows();
      choose(rows, Number(args[0] ?? 1) - 1);
      return true;
    }
    case 'page.go': {
      const page = pageNamed(args[0] ?? '');
      if (page) openPage(page);
      else say(`No page ‘${args.join(' ')}’: :tabn takes 1 to 9 or a page's name.`);
      return true;
    }
    case 'page.next':
    case 'page.prev':
    case 'page.first':
    case 'page.last': {
      const here = Math.max(0, ORDER.indexOf(store.view));
      const n = ORDER.length;
      const to =
        call.action === 'page.next'
          ? (here + 1) % n
          : call.action === 'page.prev'
            ? (here + n - 1) % n
            : call.action === 'page.first'
              ? 0
              : n - 1;
      openPage(ORDER[to]);
      return true;
    }
    case 'app.help':
      store.keysHelpOpen = true;
      return true;
    case 'app.review':
      if (review.phase === 'idle') void review.start({ surface: 'card' });
      return true;
    case 'app.context':
      if (args.length) void store.act(null, ['context', ...args]).then(() => store.loadConfig());
      else void focusField('main select.context');
      return true;
    case 'app.reminders':
      store.notifyOpen = true;
      return true;
    case 'app.sync':
    case 'app.reload':
      window.dispatchEvent(new CustomEvent('meghnaad:refresh'));
      return true;
    case 'app.commandLine':
      store.requestPrompt(':');
      return true;
    case 'app.undo':
      // `undo` asks first: it is put on the command line, for Enter.
      store.requestPrompt('undo');
      return true;
    case 'app.add':
    case 'tasks.add':
      if (args.length) void store.act(store.live, ['add', ...args]);
      else store.quickAdd();
      return true;
    case 'app.filter':
    case 'tasks.filter':
      if (args.length) store.focusReport(store.report, args);
      else void focusField(store.view === 'tasks' ? '#fb-filter' : 'main .filter input');
      return true;
    case 'tasks.report':
      if (args.length) {
        if (store.config?.reports.some((r) => r.name === args[0])) store.focusReport(args[0], []);
        else say(`No report named ‘${args[0]}’.`);
      } else void focusField('#fb-report');
      return true;
  }
  if (!call.action.startsWith('tasks.')) return false;
  if (store.view !== 'tasks') {
    say(`${call.action} works on the Tasks page.`);
    return true;
  }
  const row = chosen();
  switch (call.action) {
    case 'tasks.mark':
      if (row) {
        selection.toggle(row.uuid);
        const { rows, at } = tableRows();
        choose(rows, at + 1);
      }
      return true;
    case 'tasks.info':
      if (row) store.openDetail(row.uuid, store.live);
      return true;
    case 'tasks.done':
      void onTasks(['done']);
      return true;
    case 'tasks.delete': {
      const uuids = targets();
      if (uuids.length && confirm(uuids.length > 1 ? `Delete ${uuids.length} tasks?` : `Delete ‘${row?.description}’?`))
        void onTasks(['delete'], uuids);
      return true;
    }
    case 'tasks.duplicate':
      void onTasks(['duplicate']);
      return true;
    case 'tasks.toggle':
      if (selection.count) say('With tasks ticked: :start starts them, :stop stops them.');
      else if (row) void onTasks([row.start != null && row.end == null ? 'stop' : 'start'], [row.uuid]);
      return true;
    case 'tasks.start':
    case 'tasks.stop': {
      // On ticked tasks, those already started (or not started) are passed over.
      const start = call.action === 'tasks.start';
      const r = store.live?.result;
      const rows = r?.kind === 'report' ? r.rows : [];
      const uuids = targets().filter((u) => {
        const t = rows.find((x) => x.uuid === u);
        return !t || (t.start != null && t.end == null) !== start;
      });
      if (uuids.length) void onTasks([start ? 'start' : 'stop'], uuids);
      else say(start ? 'They are all started.' : 'None of them is started.');
      return true;
    }
    case 'tasks.modify':
    case 'tasks.annotate':
    case 'tasks.denotate':
      if (args.length) void onTasks([call.action.slice(6), ...args]);
      else if (call.action === 'tasks.modify' && row) store.editing = { row, from: store.live };
      else if (row) store.requestPrompt(`${row.id ?? row.uuid} ${call.action.slice(6)} `);
      return true;
    case 'tasks.open':
      if (row) openLink(row);
      return true;
    case 'tasks.clearCompleted':
      if (confirm('Delete the completed tasks of this report?'))
        void store.act(store.live, [...splitWords(store.filter), 'status:completed', 'delete'], { confirmed: true });
      return true;
  }
  say(`${call.action} is the terminal app's.`);
  return true;
}
