<script lang="ts">
  import { batches, filterWords, promptFor, type Pick } from './bulk';
  import BulkModify from './BulkModify.svelte';
  import { Check, Pencil, Terminal, Trash2, X } from './icons';
  import { selection } from './selection.svelte';
  import { store } from './store.svelte';

  // The ticked tasks, as the table shows them (their numbers come from the rows), in the order they were ticked.
  const picks = $derived.by((): Pick[] => {
    const r = store.live?.result;
    if (r?.kind !== 'report') return [];
    const byUuid = new Map(r.rows.map((row) => [row.uuid, row]));
    return selection.uuids.flatMap((u) => {
      const row = byUuid.get(u);
      return row ? [{ uuid: u, id: row.id }] : [];
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
  async function run(words: string[]): Promise<boolean> {
    busy = true;
    try {
      for (const batch of batches(picks)) {
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
      title="Put these tasks at the prompt and type the rest of the command"><Terminal size={15} /> Command</button
    >
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
