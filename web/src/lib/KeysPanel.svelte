<script lang="ts">
  import { untrack } from 'svelte';
  import { Check, Plus, Search, X } from './icons';
  import {
    checkCommand,
    chordOf,
    keyBindings,
    keyCommands,
    keyLabel,
    parseKey,
    withBinding,
    withoutKey,
    type BadKey,
    type Binding,
    type KeySection,
  } from './keymap';
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
      b.command.toLowerCase().includes(t) ||
      b.about.toLowerCase().includes(t) ||
      keys.some((k) => k.toLowerCase() === t || keyLabel(k).toLowerCase() === t)
    );
  }
  // The commands a key can run in place of an action, with their words (spec 0020): what each does and takes, and a link
  // to more of it.
  const commands = $derived(
    keyCommands(store.commandsDoc).filter((c) => {
      const t = query.trim().toLowerCase();
      // The name whole, as typed (`denot`), not as written (`den[otate]`).
      return !t || [c.written.replace(/[[\]]/g, ''), c.what].some((x) => x.toLowerCase().includes(t));
    }),
  );
  const rowsOf = (actions: Binding[]) =>
    actions.filter((b) => (b.keys !== null || (showDefaults && b.defaults.length > 0)) && matches(b, query));

  /** `next` as the draft, its other keys in the order they were (the engine sorts the ones it writes). */
  function keep(next: Settings) {
    done = false;
    const out: Settings = {};
    for (const k of Object.keys(draft)) if (k in next) out[k] = next[k];
    for (const k of Object.keys(next)) if (!(k in out)) out[k] = next[k];
    draft = out;
  }
  /** The keys of what runs `runs` in a section set (`[]` for none) or taken out (`null`), bound in `group`. */
  function change(section: KeySection, runs: string, keys: string[] | null, group: string | null = null) {
    keep(withBinding($state.snapshot(draft) as Settings, store.keymapDefaults, section, runs, keys, group));
  }
  /** A key that runs no command, taken out of the draft. */
  function takeOut(bad: BadKey) {
    keep(withoutKey($state.snapshot(draft) as Settings, store.keymapDefaults, bad));
  }
  const at = (b: BadKey) => `${b.section === 'general' ? 'keys' : `${b.section}.keys`}.${b.group}`;

  // Adding: the section, its search, the preset picked (none for a command of one's own), the command it runs as
  // typed, where it acts when it can act in several places, and the key for it.
  let adding = $state<KeySection | null>(null);
  let find = $state('');
  let picked = $state<Binding | null>(null);
  let writing = $state(false);
  let runs = $state('');
  let where = $state<string | null>(null);
  let recording = $state(false);
  let typed = $state('');
  let pressed = $state<string | null>(null);
  const typedKey = $derived(typed.trim() ? parseKey(typed.trim()) : null);
  const newKey = $derived(pressed ?? (typedKey && 'key' in typedKey ? typedKey.key : null));

  function openAdd(section: KeySection) {
    adding = section;
    find = '';
    picked = null;
    writing = false;
    resetKey();
  }
  /** The form for a preset, its command to start from; or, with none, for a command of one's own. */
  function pick(b: Binding | null) {
    picked = b;
    writing = true;
    runs = b?.command || b?.action || '';
    where = null;
  }
  // The command as typed, checked by the engine: what it runs, or why it is no command (spec 0025).
  const checked = $derived(writing && runs.trim() ? checkCommand(runs, store.keymapDefaults) : null);
  const runsOk = $derived(checked && 'call' in checked ? checked : null);
  // Where it is bound: the place chosen, else the preset's own, else the first it acts in.
  const place = $derived.by(() => {
    const places = runsOk?.places ?? [];
    return (
      places.find((p) => p.group === where) ?? places.find((p) => p.action === picked?.action) ?? places[0] ?? null
    );
  });
  // What it is listed as: the action alone, or the command as typed when it has words.
  const row = $derived(
    runsOk && place && !runsOk.call.args?.length && runsOk.call.count == null ? place.action : runs.trim(),
  );
  const rowNow = $derived(
    adding ? bindings.sections.find((s) => s.id === adding)?.actions.find((b) => b.action === row) : undefined,
  );
  function resetKey() {
    recording = false;
    typed = '';
    pressed = null;
  }
  function closeAdd() {
    adding = null;
    picked = null;
    writing = false;
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
    if (!adding || !runsOk || !place || !newKey) return;
    const now = rowNow?.keys ?? [];
    change(adding, runs.trim(), now.includes(newKey) ? now : [...now, newKey], place.group);
    closeAdd();
  }
  function noKey() {
    if (!adding || !runsOk || !place) return;
    change(adding, runs.trim(), [], place.group);
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
  {#if bindings.bad.length}
    <div class="warn" role="alert">
      <p>These keys run no command, so they do nothing. Take them out, or fix them in <code>settings.json</code>:</p>
      <ul>
        {#each bindings.bad as b (`${b.section} ${b.group} ${b.key}`)}
          <li>
            <span
              ><kbd>{b.key}</kbd> in <code>{at(b)}</code> runs <code>{b.runs}</code>:
              <span class="why">{b.why}</span></span
            >
            <button class="small" onclick={() => takeOut(b)} aria-label="Take out {b.key} in {at(b)}">Take out</button>
          </li>
        {/each}
      </ul>
    </div>
  {/if}
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
          {#if !writing}
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
                  <button class="result" onclick={() => pick(b)}>
                    <span
                      ><code>{b.command || b.action}</code>
                      {#if b.command && b.command !== b.action}<span class="dim">{b.action}</span>{/if}
                      <span class="dim">{b.about}</span></span
                    >
                    <span>{@render keys(b.keys ?? b.defaults, b.keys === null)}</span>
                  </button>
                </li>
              {:else}
                <li class="dim">No action matches.</li>
              {/each}
              <li>
                <button class="result own" onclick={() => pick(null)}>
                  <span><Plus size={14} /> Add a command <span class="dim">of your own, with its words</span></span>
                </button>
              </li>
            </ul>
            <div class="row"><button onclick={closeAdd}>Cancel</button></div>
          {:else}
            <label class="runs">
              <span>Command</span>
              <!-- svelte-ignore a11y_autofocus -->
              <input
                class="mono"
                placeholder="filter +home"
                aria-label="Command"
                bind:value={runs}
                autofocus={!picked}
                spellcheck="false"
                autocomplete="off"
                onkeydown={(e) => e.key === 'Escape' && closeAdd()}
              />
            </label>
            {#if checked && 'problem' in checked}
              <p class="err" role="alert">{checked.problem}</p>
            {:else if runsOk && place}
              <p class="dim">
                Runs <code>{place.action}</code>{#if runsOk.call.args?.length}&#32;with
                  <code>{runsOk.call.args.join(' ')}</code>{/if}{#if runsOk.about}: {runsOk.about}{/if}
              </p>
              {#if runsOk.places.length > 1}
                <label class="row">
                  <span>Where</span>
                  <select aria-label="Where" value={place.group} onchange={(e) => (where = e.currentTarget.value)}>
                    {#each runsOk.places as p (p.group)}<option value={p.group}>{p.group}: {p.action}</option>{/each}
                  </select>
                </label>
              {/if}
              {#if rowNow?.keys === null && rowNow.defaults.length}
                <p class="dim">Replaces its default {@render keys(rowNow.defaults, true)}</p>
              {/if}
            {:else}
              <p class="dim">
                A command, with its words: <code>done</code>, <code>filter +home</code>, <code>sort due-</code>.
              </p>
            {/if}
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
              <button class="primary" disabled={!newKey || !runsOk} onclick={addKey}>Add key</button>
              <button onclick={noKey} disabled={!runsOk} title="Leave it without a key">No key</button>
              <button onclick={() => ((writing = false), (picked = null), resetKey())}>Back</button>
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
                  <code>{b.command || b.action}</code>
                  {#if b.command && b.command !== b.action}<span class="dim">{b.action}</span>{/if}
                  {#if b.about}<span class="dim">· {b.about}</span>{/if}
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

  <section class="group" aria-label="Commands">
    <h2>Commands</h2>
    <p class="dim">
      A key can run one of these, with its words, in place of an action: <code>"g h": "filter +home"</code> under
      <code>tasks</code> runs <code>:filter +home</code>. The part in brackets may be left out, as in vim.
    </p>
    {#if commands.length}
      <table class="keys">
        <tbody>
          {#each commands as c (c.written)}
            <tr>
              <td class="k"><kbd>{c.written}</kbd></td>
              <td>
                <span class="dim">{c.what}</span>
                {#if c.link}<a href={c.link} target="_blank" rel="noopener">more</a>{/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {:else}
      <p class="dim none">Nothing matches.</p>
    {/if}
  </section>

  {#if bindings.problems.length}
    <ul class="err" role="alert">
      {#each bindings.problems as line, i (i)}<li>{line}</li>{/each}
    </ul>
  {/if}
  <p class="dim">
    Kept in <code>settings.json</code>: <code>keys</code> for every front end, the others for one alone, each key under its
    group, saved as the command it runs. A key you set here replaces the defaults of what it runs, and is taken from any action
    that had it.
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
  .result.own {
    color: var(--accent);
  }
  .runs {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .runs input {
    flex: 1;
    min-width: 0;
  }
  .warn {
    border: 1px solid var(--warn);
    border-radius: 6px;
    padding: 8px 10px;
    margin-bottom: 14px;
    font-size: 13px;
  }
  .warn p {
    margin: 0 0 6px;
  }
  .warn ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .warn li {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 10px;
    padding: 3px 0;
  }
  .why {
    color: var(--dim);
  }
</style>
