<script lang="ts">
  import { untrack } from 'svelte';
  import { Check, Plus, Search, X } from './icons';
  import { chordOf, keyBindings, keyLabel, parseKey, withBinding, type Binding, type KeySection } from './keymap';
  import { store } from './store.svelte';

  /** There are edits not yet applied (the tab shows a dot). */
  let { ondirty }: { ondirty?: (dirty: boolean) => void } = $props();

  const TITLES: Record<KeySection, { label: string; at: string }> = {
    general: { label: 'General', at: 'keys' },
    web: { label: 'Web', at: 'web.keys' },
    mobile: { label: 'Mobile', at: 'mobile.keys' },
    tui: { label: 'Terminal', at: 'tui.keys' },
  };

  type Settings = Record<string, unknown>;
  // One text for a settings object, whatever order its keys are in (the engine writes them sorted).
  const canon = (v: unknown): string =>
    JSON.stringify(v, (_k, x: unknown) =>
      x && typeof x === 'object' && !Array.isArray(x)
        ? Object.fromEntries(Object.entries(x).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)))
        : x,
    );

  const saved = $derived<Settings>(store.appSettings?.raw ?? {});
  let draft = $state<Settings>(untrack(() => $state.snapshot(saved) as Settings));
  let busy = $state(false);
  let error = $state<string | null>(null);
  let done = $state(false);

  const edited = $derived(canon(draft) !== canon(saved));
  $effect(() => {
    ondirty?.(edited);
  });

  // Saved somewhere else (another tab, the file): follow it, unless there are edits here.
  let before = untrack(() => canon(saved));
  $effect(() => {
    const now = canon(saved);
    const untouched = untrack(() => canon(draft)) === before;
    const changed = now !== before;
    before = now;
    if (changed && untouched) draft = $state.snapshot(saved) as Settings;
  });

  const bindings = $derived(keyBindings(draft, store.keymapDefaults));

  // The defaults are shown or not, as this browser last had it.
  const SHOW_KEY = 'tw-web-keys-defaults';
  let showDefaults = $state(
    (() => {
      try {
        return localStorage.getItem(SHOW_KEY) === '1';
      } catch {
        return false;
      }
    })(),
  );
  function toggleDefaults(on: boolean) {
    showDefaults = on;
    try {
      localStorage.setItem(SHOW_KEY, on ? '1' : '0');
    } catch {
      // Not remembered: it is still shown now.
    }
  }

  let query = $state('');
  /** `b` matches what is typed: its name, what it does, or one of its keys. */
  function matches(b: Binding, q: string): boolean {
    const t = q.trim().toLowerCase();
    if (!t) return true;
    const keys = b.keys ?? b.defaults;
    return (
      b.action.toLowerCase().includes(t) ||
      b.about.toLowerCase().includes(t) ||
      keys.some((k) => k.toLowerCase() === t || keyLabel(k).toLowerCase() === t)
    );
  }
  const rowsOf = (actions: Binding[]) =>
    actions.filter((b) => (b.keys !== null || (showDefaults && b.defaults.length > 0)) && matches(b, query));

  /** `settings` with one change, its other keys in the order they were (the engine sorts the ones it writes). */
  function change(section: KeySection, action: string, keys: string[] | null) {
    done = false;
    const next = withBinding($state.snapshot(draft) as Settings, section, action, keys);
    const out: Settings = {};
    for (const k of Object.keys(draft)) if (k in next) out[k] = next[k];
    for (const k of Object.keys(next)) if (!(k in out)) out[k] = next[k];
    draft = out;
  }

  // Adding: the section, its search, the action picked and the key for it.
  let adding = $state<KeySection | null>(null);
  let find = $state('');
  let picked = $state<Binding | null>(null);
  let recording = $state(false);
  let typed = $state('');
  let pressed = $state<string | null>(null);
  const typedKey = $derived(typed.trim() ? parseKey(typed.trim()) : null);
  const newKey = $derived(pressed ?? (typedKey && 'key' in typedKey ? typedKey.key : null));

  function openAdd(section: KeySection) {
    adding = section;
    find = '';
    picked = null;
    resetKey();
  }
  function resetKey() {
    recording = false;
    typed = '';
    pressed = null;
  }
  function closeAdd() {
    adding = null;
    picked = null;
    resetKey();
  }
  const found = $derived(
    adding ? (bindings.sections.find((s) => s.id === adding)?.actions ?? []).filter((b) => matches(b, find)) : [],
  );

  function record(e: KeyboardEvent) {
    // The key is the binding: nothing else on the page may take it.
    e.preventDefault();
    e.stopPropagation();
    if (e.key === 'Escape') {
      recording = false;
      return;
    }
    const chord = chordOf(e);
    if (!chord) return;
    pressed = chord;
    typed = '';
    recording = false;
  }

  function addKey() {
    if (!adding || !picked || !newKey) return;
    const now = picked.keys ?? [];
    change(adding, picked.action, now.includes(newKey) ? now : [...now, newKey]);
    closeAdd();
  }
  function noKey() {
    if (!adding || !picked) return;
    change(adding, picked.action, []);
    closeAdd();
  }

  async function apply() {
    busy = true;
    error = null;
    done = false;
    try {
      await store.saveKeys($state.snapshot(draft) as Settings);
      done = true;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }
</script>

{#snippet keys(list: string[], muted: boolean)}
  {#each list as k, i (i)}<kbd class:muted>{keyLabel(k)}</kbd>{/each}
{/snippet}

<section aria-label="Keybindings">
  <div class="top">
    <label class="search">
      <Search size={14} />
      <input type="search" placeholder="Search keys" aria-label="Search keys" bind:value={query} />
    </label>
    <label class="check">
      <input type="checkbox" checked={showDefaults} onchange={(e) => toggleDefaults(e.currentTarget.checked)} />
      Show defaults
    </label>
  </div>

  {#each bindings.sections as s (s.id)}
    {@const rows = rowsOf(s.actions)}
    <section class="group" aria-label={TITLES[s.id].label}>
      <div class="head">
        <h2>{TITLES[s.id].label} <code class="dim">{TITLES[s.id].at}</code></h2>
        <button class="small" onclick={() => openAdd(s.id)} aria-label="Add a key to {TITLES[s.id].label}">
          <Plus size={14} /> Add
        </button>
      </div>

      {#if adding === s.id}
        <div class="add" role="group" aria-label="Add a key">
          {#if !picked}
            <!-- svelte-ignore a11y_autofocus -->
            <input
              type="search"
              placeholder="Search actions"
              aria-label="Search actions"
              bind:value={find}
              autofocus
              onkeydown={(e) => e.key === 'Escape' && closeAdd()}
            />
            <ul class="found">
              {#each found as b (b.action)}
                <li>
                  <button class="result" onclick={() => (picked = b)}>
                    <span><code>{b.action}</code> <span class="dim">{b.about}</span></span>
                    <span>{@render keys(b.keys ?? b.defaults, b.keys === null)}</span>
                  </button>
                </li>
              {:else}
                <li class="dim">No action matches.</li>
              {/each}
            </ul>
            <div class="row"><button onclick={closeAdd}>Cancel</button></div>
          {:else}
            <p>
              <code>{picked.action}</code> <span class="dim">{picked.about}</span>
              {#if picked.keys === null && picked.defaults.length}
                <br /><span class="dim">Replaces its default {@render keys(picked.defaults, true)}</span>
              {/if}
            </p>
            <div class="row">
              <button
                class="capture"
                class:on={recording}
                onclick={() => (recording = true)}
                onkeydown={(e) => recording && record(e)}
                onblur={() => (recording = false)}
              >
                {#if recording}Press a key… (Esc to stop){:else if pressed}<kbd>{keyLabel(pressed)}</kbd>{:else}Press a
                  key{/if}
              </button>
              <span class="dim">or type it</span>
              <input
                class="typed"
                placeholder="ctrl+k"
                aria-label="Key"
                bind:value={typed}
                oninput={() => (pressed = null)}
                onkeydown={(e) => e.key === 'Enter' && addKey()}
              />
            </div>
            {#if typedKey && 'problem' in typedKey}<p class="err">{typedKey.problem}</p>{/if}
            <div class="row">
              <button class="primary" disabled={!newKey} onclick={addKey}>Add key</button>
              <button onclick={noKey} title="Leave it without a key">No key</button>
              <button onclick={() => ((picked = null), resetKey())}>Back</button>
              <button onclick={closeAdd}>Cancel</button>
            </div>
          {/if}
        </div>
      {/if}

      {#if rows.length}
        <table class="keys">
          <tbody>
            {#each rows as b (b.action)}
              <tr class:default={b.keys === null}>
                <td class="k">
                  {#if b.keys === null}{@render keys(b.defaults, true)}
                  {:else if b.keys.length}{@render keys(b.keys, false)}
                  {:else}<span class="dim">no key</span>{/if}
                </td>
                <td>
                  <code>{b.action}</code>
                  {#if b.about}<span class="dim">{b.about}</span>{:else}<span class="err">no such action</span>{/if}
                  {#if b.keys !== null && showDefaults && b.defaults.length}
                    <span class="dim">· default {@render keys(b.defaults, true)}</span>
                  {/if}
                </td>
                <td class="x">
                  {#if b.keys !== null}
                    <button
                      class="icon"
                      title="Take it out (back to the default)"
                      aria-label="Take out {b.action}"
                      onclick={() => change(s.id, b.action, null)}><X size={14} /></button
                    >
                  {/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      {:else}
        <p class="dim none">{query.trim() ? 'Nothing matches.' : 'None of your own.'}</p>
      {/if}
    </section>
  {/each}

  {#if bindings.problems.length}
    <ul class="err" role="alert">
      {#each bindings.problems as line, i (i)}<li>{line}</li>{/each}
    </ul>
  {/if}
  <p class="dim">
    Kept in <code>settings.json</code>: <code>keys</code> for every front end, the others for one alone. A key you set replaces
    the action's defaults, and is taken from any action that had it.
  </p>
  {#if error}<p class="err" role="alert">{error}</p>{/if}
  <div class="row">
    <button class="primary" onclick={apply} disabled={busy || !edited}>Apply</button>
    {#if done}<span class="ok"><Check size={14} /> Saved</span>{/if}
  </div>
</section>

<style>
  .top {
    display: flex;
    flex-wrap: wrap;
    gap: 8px 16px;
    align-items: center;
    margin-bottom: 14px;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: 1 1 220px;
    color: var(--dim);
  }
  .search input {
    flex: 1;
    min-width: 0;
  }
  .check {
    display: flex;
    gap: 6px;
    align-items: center;
    font-size: 13px;
  }
  .group + .group {
    margin-top: 18px;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    border-bottom: 1px solid var(--line);
    padding-bottom: 4px;
    margin-bottom: 6px;
  }
  h2 {
    font-size: 15px;
    margin: 0;
  }
  h2 code {
    font-size: 12px;
    font-weight: normal;
  }
  .keys {
    border-collapse: collapse;
    font-size: 13px;
    width: 100%;
  }
  .keys td {
    padding: 3px 12px 3px 0;
    vertical-align: top;
  }
  .keys td.k {
    white-space: nowrap;
    width: 1%;
  }
  .keys td.x {
    width: 1%;
    padding-right: 0;
  }
  .keys code {
    margin-right: 6px;
  }
  tr.default code {
    color: var(--dim);
  }
  kbd {
    display: inline-block;
    font: 12px var(--mono, monospace);
    padding: 1px 6px;
    margin-right: 3px;
    border: 1px solid var(--line);
    border-radius: 4px;
    background: var(--panel-2);
  }
  kbd.muted {
    color: var(--dim);
    background: transparent;
    border-style: dashed;
  }
  .none {
    font-size: 13px;
    margin: 4px 0;
  }
  .add {
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 10px;
    margin-bottom: 8px;
    background: var(--panel-2);
    font-size: 13px;
  }
  .add > input {
    width: 100%;
    box-sizing: border-box;
  }
  .found {
    list-style: none;
    margin: 6px 0;
    padding: 0;
    max-height: 260px;
    overflow-y: auto;
  }
  .result {
    display: flex;
    justify-content: space-between;
    gap: 10px;
    width: 100%;
    text-align: left;
    border-color: transparent;
    background: transparent;
  }
  .result:hover,
  .result:focus-visible {
    background: var(--panel);
    border-color: var(--line);
  }
  .capture.on {
    border-color: var(--accent);
  }
  .typed {
    width: 120px;
  }
  .icon {
    padding: 2px 4px;
    line-height: 0;
  }
</style>
