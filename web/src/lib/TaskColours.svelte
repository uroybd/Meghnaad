<script lang="ts">
  import { getTaskrcText, putRcFile } from './api';
  import { ROOT } from './rcpaths';
  import { store } from './store.svelte';
  import { applyTheme, colourOff, fetchTheme, THEMES, withColour } from './themes';

  /**
   * The colours of tasks, which are the `color.*` lines of the main taskrc: a theme replaces them, and the switch is
   * `color=off`. Each change is saved at once, as one save of the taskrc, so Restore previous on the taskrc tab undoes it.
   */
  let text = $state<string | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let said = $state<string | null>(null);

  // Read the main taskrc once shown, and again when the settings change elsewhere.
  $effect(() => {
    void store.config;
    getTaskrcText()
      .then((t) => (text = t))
      .catch((e: unknown) => (error = e instanceof Error ? e.message : String(e)));
  });

  const off = $derived(text === null ? false : colourOff(text));

  async function save(next: string, what: string) {
    busy = true;
    error = null;
    said = null;
    try {
      const r = await putRcFile(ROOT, next);
      text = r.text;
      said = `${what} Restore previous on the taskrc tab undoes it.`;
      await store.settingsChanged();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  async function pickTheme(e: Event) {
    const el = e.currentTarget as HTMLSelectElement;
    const id = el.value;
    el.value = '';
    if (!id || text === null) return;
    const label = THEMES.find((t) => t.id === id)?.label ?? id;
    try {
      await save(applyTheme(text, id, id === 'default' ? '' : await fetchTheme(id)), `Saved the ${label} colours.`);
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    }
  }
</script>

<section class="colours" aria-label="Task colours">
  <h2>Task colours</h2>
  <p class="dim small">
    The <code>color.*</code> lines of your taskrc. A theme replaces them; the Meghnaad theme is what applies when there are
    none. Saved at once.
  </p>
  <div class="row">
    <label
      >Colour theme
      <select onchange={pickTheme} disabled={busy || text === null} aria-label="Colour theme">
        <option value="">choose…</option>
        {#each THEMES as t (t.id)}<option value={t.id} title={t.note}>{t.label}</option>{/each}
      </select>
    </label>
    <label class="check"
      ><input
        type="checkbox"
        checked={!off}
        disabled={busy || text === null}
        onchange={(e) => {
          const on = e.currentTarget.checked;
          if (text !== null) void save(withColour(text, on), on ? 'Colours are on.' : 'Colours are off.');
        }}
      />
      Colours</label
    >
  </div>
  {#if error}<p class="err" role="alert">{error}</p>{/if}
  {#if said}<p class="ok" role="status">{said}</p>{/if}
</section>

<style>
  h2 {
    margin: 0 0 4px;
    font-size: 16px;
  }
  .colours {
    margin-bottom: 18px;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 16px;
  }
  .row select {
    margin-left: 6px;
  }
  .check {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .small {
    font-size: 12.5px;
  }
  code {
    font-family: var(--mono);
  }
</style>
