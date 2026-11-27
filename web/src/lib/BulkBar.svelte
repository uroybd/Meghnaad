<script lang="ts">
  import { commandStepsOn, udaNames } from './kanban';
  import { batches, filterWords, promptFor, type Pick } from './bulk';
  import BulkModify from './BulkModify.svelte';
  import type { TaskCommand } from './appsettings';
  import { Check, Pencil, Terminal, Trash2, X } from './icons';
  import MoreMenu from './MoreMenu.svelte';
  import { selection } from './selection.svelte';
  import { store } from './store.svelte';
  import { runSequence } from './compound';

  // The ticked tasks, as the table shows them (their numbers come from the rows), in the order they were ticked.
  const picks = $derived.by((): Pick[] => {
    const r = store.live?.result;
    if (r?.kind !== 'report') return [];
    const byUuid = new Map(r.rows.map((row) => [row.uuid, row]));
    return selection.uuids.flatMap((u) => {
      const row = byUuid.get(u);
      return row ? [{ uuid: u, id: row.id, iterative: row.status === 'iterative' }] : [];
    });
  });

  let busy = $state(false);
  let confirmDelete = $state(false);
  let modifying = $state(false);

  /**
   * Run `words` on the ticked tasks, a batch at a time (the Worker takes a command of a limited size). The click is
   * the answer to Taskwarrior's "are you sure?" questions, so they are answered yes; questions about recurring tasks
   * and dependencies are still asked. Returns whether everything went through.
   */
  async function run(words: string[], on: Pick[] = picks): Promise<boolean> {
    busy = true;
    try {
      for (const batch of batches(on)) {
        const uuids = batch.map((t) => t.uuid);
        const res = await store.act(store.live, [...filterWords(batch), ...words], {
          approved: uuids,
          confirmed: true,
        });
        if (!res?.wrote) return false;
      }
      return true;
    } finally {
      busy = false;
    }
  }

  async function complete() {
    if (await run(['done'])) selection.clear();
  }

  // The second click is the answer to "Delete N tasks?"; with `confirmation` off, the first click deletes.
  async function del() {
    if (store.confirmation && !confirmDelete) {
      confirmDelete = true;
      setTimeout(() => (confirmDelete = false), 3000);
      return;
    }
    confirmDelete = false;
    if (await run(['delete'])) selection.clear();
  }

  async function modify(words: string[]): Promise<boolean> {
    const ok = await run(['modify', ...words]);
    if (ok) selection.clear();
    return ok;
  }

  // A task command from settings.json, run on the ticked tasks.
  // An iterative task (`+ITERATIVE`) gets it without its `status:`, which Taskwarrior refuses on one; the rest as it is.
  async function runCommand(c: TaskCommand) {
    const udas = udaNames(store.config);
    const groups = [false, true]
      .map((iterative) => ({
        on: picks.filter((p) => !!p.iterative === iterative),
        steps: commandStepsOn(c, [], iterative, udas),
      }))
      .filter((g) => g.on.length && g.steps.length);
    if (!groups.length) {
      store.notify(`${c.name} only changes the status, which an iterative task keeps.`, 'err');
      return;
    }
    for (const g of groups) {
      const ok = await runSequence(
        g.steps.map((s) => s.join),
        (i) => run(g.steps[i].words, g.on),
      );
      if (!ok) return;
    }
    selection.clear();
  }

  // Name the tasks at the prompt and leave the rest to the user.
  function command() {
    store.requestPrompt(promptFor(picks));
  }
</script>

{#if picks.length > 1}
  <div class="bulk row" role="toolbar" aria-label="Actions for the {picks.length} selected tasks" data-testid="bulkbar">
    <strong class="count">{picks.length} selected</strong>
    <button class="btn" disabled={busy} onclick={complete}><Check size={15} /> Complete</button>
    <button class="btn danger" disabled={busy} onclick={del}>
      <Trash2 size={15} />
      {confirmDelete ? `Delete ${picks.length}? Click again` : 'Delete'}
    </button>
    <button class="btn" disabled={busy} onclick={() => (modifying = true)}><Pencil size={15} /> Modify…</button>
    <button
      class="btn"
      disabled={busy}
      onclick={command}
      aria-label="Send to CLI"
      title="Send to CLI: put these tasks at the prompt and type the rest of the command"
      ><Terminal size={15} /> CLI</button
    >
    <MoreMenu
      commands={store.menuCommands}
      iterative={picks.length > 0 && picks.every((p) => p.iterative)}
      disabled={busy}
      onrun={runCommand}
      label="More actions for the selected tasks"
    />
    <span class="grow"></span>
    <button class="ghost btn" onclick={() => selection.clear()} aria-label="Clear the selection"
      ><X size={15} /> Clear</button
    >
  </div>
{/if}

{#if modifying}
  <BulkModify count={picks.length} onapply={modify} onclose={() => (modifying = false)} />
{/if}

<style>
  .bulk {
    position: sticky;
    top: 0;
    z-index: 6;
    flex-wrap: wrap;
    gap: 8px;
    margin: 0 0 8px;
    padding: 8px 10px;
    background: var(--panel);
    border: 1px solid var(--accent);
    border-radius: var(--radius);
    box-shadow: 0 4px 14px rgb(0 0 0 / 0.12);
  }
  .count {
    margin-right: 4px;
  }
  .btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  @media (max-width: 760px) {
    .btn {
      min-height: 40px;
    }
  }
</style>
