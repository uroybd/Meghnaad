<script lang="ts">
  import { ROOT } from './rcpaths';
  import { settingsData } from './settingsData.svelte';
  import { store } from './store.svelte';
  import { chooseTheme, chosenTheme, colourOff, fetchTheme, THEMES, withColour } from './themes';

  /**
   * The colours of tasks. A theme is saved as its own file, `themes/<id>.rc`, which the main taskrc includes in place of
   * its own colour lines; the switch is `color=off` in the main taskrc. Each change is saved at once, as one save (the
   * theme's file and the main taskrc together), so Restore previous on the taskrc tab undoes it.
   */
  let text = $state<string | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let said = $state<string | null>(null);

  // Read the main taskrc once shown, and again when the settings change elsewhere (shared with the taskrc tab).
  $effect(() => {
    void settingsData.rev;
    settingsData
      .file(ROOT)
      .get()
      .then((t) => (text = t))
      .catch((e: unknown) => (error = e instanceof Error ? e.message : String(e)));
  });

  const off = $derived(text === null ? false : colourOff(text));
  const chosen = $derived(text === null ? null : chosenTheme(text));

  async function save(next: string, what: string, theme: { path: string; text: string } | null = null) {
    busy = true;
    error = null;
    said = null;
    try {
      if (theme) {
        await settingsData.importRcFiles([theme, { path: ROOT, text: next }], 'merge');
        text = await settingsData.file(ROOT).get();
      } else text = (await settingsData.putRcFile(ROOT, next)).text;
      said = `${what} Restore previous on the taskrc tab undoes it.`;
      await store.settingsChanged([ROOT]);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  async function pickTheme(e: Event) {
    const id = (e.currentTarget as HTMLSelectElement).value;
    if (!id || text === null) return;
    const label = THEMES.find((t) => t.id === id)?.label ?? id;
    try {
      const { main, theme } = chooseTheme(text, id, id === 'default' ? '' : await fetchTheme(id));
      await save(main, theme ? `Saved the ${label} colours, in ${theme.path}.` : `Saved the ${label} colours.`, theme);
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    }
  }
</script>

<section class="colours" aria-label="Task colours">
  <h2>Task colours</h2>
  <p class="dim small">
    A theme is kept as its own file, <code>themes/&lt;name&gt;.rc</code>, which your taskrc includes in place of its own
    <code>color.*</code> lines; the Meghnaad theme is what applies when there are none. Saved at once.
  </p>
  <div class="row">
    <label
      >Colour theme
      <select value={chosen ?? ''} onchange={pickTheme} disabled={busy || text === null} aria-label="Colour theme">
        <option value="">{text !== null && !chosen ? 'your own, or the default' : 'choose…'}</option>
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
