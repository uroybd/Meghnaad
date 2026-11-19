<script lang="ts">
  import type { Snippet } from 'svelte';
  import { Plus } from './icons';
  import { bindingMatches, checkCommand, chordOf, keyLabel, parseKey, type Binding } from './keymap';
  import { store } from './store.svelte';

  let {
    actions,
    keys,
    onchange,
    onclose,
  }: {
    /** What the section binds: the presets to pick from, and the rows a new key joins. */
    actions: Binding[];
    /** Draws a list of keys (muted: the defaults). */
    keys: Snippet<[string[], boolean]>;
    /** Bind `runs` to `keys` (`[]`: no key) in `group`. */
    onchange: (runs: string, keys: string[], group: string) => void;
    onclose: () => void;
  } = $props();

  // The search, the preset picked (none for a command of one's own), the command it runs as typed, where it acts when it
  // can act in several places, and the key for it.
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
  const found = $derived(actions.filter((b) => bindingMatches(b, find)));

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
  const rowNow = $derived(actions.find((b) => b.action === row));
  function resetKey() {
    recording = false;
    typed = '';
    pressed = null;
  }

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
    if (!runsOk || !place || !newKey) return;
    const now = rowNow?.keys ?? [];
    onchange(runs.trim(), now.includes(newKey) ? now : [...now, newKey], place.group);
    onclose();
  }
  function noKey() {
    if (!runsOk || !place) return;
    onchange(runs.trim(), [], place.group);
    onclose();
  }
</script>

<div class="add" role="group" aria-label="Add a key">
  {#if !writing}
    <!-- svelte-ignore a11y_autofocus -->
    <input
      type="search"
      placeholder="Search actions"
      aria-label="Search actions"
      bind:value={find}
      autofocus
      onkeydown={(e) => e.key === 'Escape' && onclose()}
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
    <div class="row"><button onclick={onclose}>Cancel</button></div>
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
        onkeydown={(e) => e.key === 'Escape' && onclose()}
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
        {#if recording}Press a key… (Esc to stop){:else if pressed}<kbd>{keyLabel(pressed)}</kbd>{:else}Press a key{/if}
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
      <button onclick={onclose}>Cancel</button>
    </div>
  {/if}
</div>

<style>
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
  .result.own {
    color: var(--accent);
  }
  .capture.on {
    border-color: var(--accent);
  }
  .typed {
    width: 120px;
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
  kbd {
    display: inline-block;
    font: 12px var(--mono, monospace);
    padding: 1px 6px;
    margin-right: 3px;
    border: 1px solid var(--line);
    border-radius: 4px;
    background: var(--panel-2);
  }
</style>
