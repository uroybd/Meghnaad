<script lang="ts">
  import { untrack } from 'svelte';
  import { DEFAULT_PROGRESS, progressProblem, sameAppearance, type Appearance, type Progress } from './appsettings';
  import { Check } from './icons';
  import { store } from './store.svelte';

  /** There are edits not yet applied (the tab shows a dot). */
  let { ondirty }: { ondirty?: (dirty: boolean) => void } = $props();

  const saved = $derived(store.appearance);
  // What is typed: for each UDA shown as a bar, its min and max as text (so a half-typed number is not lost).
  type Draft = Record<string, { min: string; max: string }>;
  const toDraft = (a: Appearance): Draft =>
    Object.fromEntries(Object.entries(a.progress).map(([n, p]) => [n, { min: String(p.min), max: String(p.max) }]));
  let draft = $state<Draft>(untrack(() => toDraft(saved)));
  let busy = $state(false);
  let error = $state<string | null>(null);
  let done = $state(false);

  /** The numeric UDAs the taskrc defines, and any that are saved as bars but not defined (any more), so they can be turned off. */
  const names = $derived([
    ...new Set([
      ...Object.values(store.config?.config.udas ?? {})
        .filter((u) => u.type === 'numeric')
        .map((u) => u.name),
      ...Object.keys(draft),
    ]),
  ]);

  const range = (name: string): Progress => ({ min: Number(draft[name].min), max: Number(draft[name].max) });
  const problem = (name: string) => {
    const d = draft[name];
    if (!d) return null;
    if (d.min.trim() === '' || d.max.trim() === '') return 'min and max are needed';
    return progressProblem(range(name));
  };
  const current = $derived<Appearance>({
    progress: Object.fromEntries(Object.keys(draft).map((n) => [n, range(n)])),
  });
  const invalid = $derived(Object.keys(draft).some((n) => problem(n)));
  const edited = $derived(!sameAppearance(current, saved));
  $effect(() => {
    ondirty?.(edited);
  });

  // Saved somewhere else (another tab, the file): follow it, unless there are edits here.
  let before = untrack(() => saved);
  $effect(() => {
    const now = saved;
    const untouched = sameAppearance(
      untrack(() => current),
      before,
    );
    const changed = !sameAppearance(now, before);
    before = now;
    if (changed && untouched) draft = toDraft(now);
  });

  function toggle(name: string, on: boolean) {
    done = false;
    if (on) draft[name] = { min: String(DEFAULT_PROGRESS.min), max: String(DEFAULT_PROGRESS.max) };
    else delete draft[name];
  }

  async function apply() {
    busy = true;
    error = null;
    done = false;
    try {
      await store.saveAppearance(current);
      done = true;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }
</script>

<section aria-label="Appearance settings">
  <h2>Progress bars</h2>
  <p class="dim">
    A numeric UDA shown as a bar in reports: empty at its minimum, full at its maximum. A value outside the range fills
    or empties the bar; a task with no value shows nothing. It is kept in this app's own
    <code>settings.json</code>, not in your taskrc.
  </p>
  {#if store.appSettings?.appearanceErrors.length}
    <p class="err" role="alert">{store.appSettings.appearanceErrors.join(' ')}</p>
  {/if}
  {#if names.length === 0}
    <p class="dim">No numeric UDAs yet. Define one in the taskrc (<code>uda.progress.type=numeric</code>).</p>
  {/if}
  <ul class="udas">
    {#each names as name (name)}
      {@const on = name in draft}
      <li>
        <label class="pick">
          <input type="checkbox" checked={on} onchange={(e) => toggle(name, e.currentTarget.checked)} />
          <span class="name">{name}</span>
        </label>
        {#if on}
          <label class="num">
            <span>min</span>
            <input type="number" step="any" bind:value={draft[name].min} oninput={() => (done = false)} />
          </label>
          <label class="num">
            <span>max</span>
            <input type="number" step="any" bind:value={draft[name].max} oninput={() => (done = false)} />
          </label>
          {#if problem(name)}<span class="err" role="alert">{problem(name)}</span>{/if}
        {/if}
      </li>
    {/each}
  </ul>
  {#if error}<p class="err" role="alert">{error}</p>{/if}
  <div class="row">
    <button class="primary" onclick={apply} disabled={busy || !edited || invalid}>Apply</button>
    {#if done}<span class="ok"><Check size={14} /> Saved</span>{/if}
  </div>
</section>

<style>
  h2 {
    font-size: 16px;
    margin: 0 0 10px;
  }
  .udas {
    list-style: none;
    margin: 0 0 12px;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 12px;
  }
  .pick {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    min-width: 140px;
  }
  .name {
    font-family: var(--mono, monospace);
  }
  .num {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--dim);
  }
  .num input {
    width: 90px;
    font-size: 14px;
    color: var(--text);
  }
</style>
