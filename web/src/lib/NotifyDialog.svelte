<script lang="ts">
  import { notifier } from './notifier.svelte';
  import { store } from './store.svelte';

  let dialog: HTMLDialogElement;
  $effect(() => {
    dialog.showModal();
  });

  const s = notifier.settings;
  const close = () => (store.notifyOpen = false);
  const hours = Array.from({ length: 24 }, (_, h) => h);
  const label = (h: number) => `${String(h).padStart(2, '0')}:00`;

  const status = $derived(
    notifier.permission === 'unsupported'
      ? 'This browser (or this non-HTTPS page) can\'t show system notifications. You\'ll still get alerts inside the page.'
      : notifier.permission === 'denied'
        ? 'System notifications are blocked for this site in your browser settings. You\'ll still get alerts inside the page.'
        : notifier.permission === 'default'
          ? 'Your browser will ask for permission when you turn reminders on.'
          : 'System notifications are allowed.',
  );
</script>

<dialog bind:this={dialog} onclose={close} aria-label="Reminders">
  <h3>Reminders</h3>
  <p class="dim">
    Get a heads-up about due dates and tasks coming off <code>wait</code>. They only work while this tab is
    open: the app can't wake up in the background.
  </p>

  <label class="check">
    <input
      type="checkbox"
      checked={notifier.settings.enabled}
      onchange={(e) => (e.currentTarget.checked ? notifier.enable() : notifier.disable())}
    />
    Remind me about due dates
  </label>
  <p class="dim small" role="status">{status}</p>

  <div class="grid" class:off={!notifier.settings.enabled}>
    <label for="nl-lead">Heads-up before a timed due date</label>
    <select id="nl-lead" bind:value={notifier.settings.leadMinutes} onchange={() => notifier.save()}>
      <option value={0}>none</option><option value={5}>5 minutes</option><option value={15}>15 minutes</option>
      <option value={30}>30 minutes</option><option value={60}>1 hour</option><option value={120}>2 hours</option>
    </select>

    <label for="nl-hour">Due on a date with no time</label>
    <select id="nl-hour" bind:value={notifier.settings.allDayHour} onchange={() => notifier.save()}>
      {#each hours as h}<option value={h}>remind at {label(h)}</option>{/each}
    </select>

    <span></span>
    <label class="check">
      <input type="checkbox" bind:checked={notifier.settings.waitOver} onchange={() => notifier.save()} />
      Tell me when a waiting task comes back
    </label>
  </div>

  {#if notifier.lastError}<p class="err small">Couldn't check for reminders: {notifier.lastError}</p>{/if}

  <footer class="row">
    <button type="button" onclick={() => notifier.test()}>Send a test</button>
    <span class="grow"></span>
    <button class="primary" onclick={close}>Done</button>
  </footer>
</dialog>

<style>
  dialog { border: 1px solid var(--line); border-radius: var(--radius); background: var(--panel); color: var(--text); padding: 16px 20px; width: min(520px, 96vw); }
  dialog::backdrop { background: rgb(0 0 0 / 0.4); }
  h3 { margin: 0 0 4px; }
  .grid { display: grid; grid-template-columns: 1fr auto; gap: 8px 12px; align-items: center; margin: 10px 0; }
  .grid.off { opacity: 0.5; }
  .check { display: flex; gap: 8px; align-items: center; }
  .small { font-size: 12px; margin: 2px 0; }
  code { font-family: var(--mono); background: var(--panel-2); border-radius: 4px; padding: 0 4px; }
  footer { margin-top: 10px; }
</style>
