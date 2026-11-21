<script lang="ts">
  import { untrack } from 'svelte';
  import { Check, Plus, Search, Undo2, X } from './icons';
  import KeyAddForm from './KeyAddForm.svelte';
  import {
    bindingMatches as matches,
    keyBindings,
    keyCommands,
    keyLabel,
    withBinding,
    withoutKey,
    type BadKey,
    type Binding,
    type KeySection,
  } from './keymap';
  import { store } from './store.svelte';
  import { ensureDeviceId } from './deviceAsk.svelte';
  import { withDeviceKeys } from './deviceSettings';

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

  // Under This device (spec 0027) the keys it uses, everyone's with its own over them; saved as what differs.
  const device = $derived(store.settingsDevice);
  const everyones = $derived<Settings>(store.appSettings?.raw ?? {});
  const saved = $derived<Settings>(device ? (store.effectiveSettings?.raw ?? {}) : everyones);
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
  // Each action's keys in everyone's settings, and in what this device uses: where they differ, the keys are its own.
  const keysOf = (b: Binding | undefined) => JSON.stringify(b ? (b.keys ?? b.defaults) : null);
  const index = (settings: Settings) =>
    new Map(
      keyBindings(settings, store.keymapDefaults).sections.flatMap((sec) =>
        sec.actions.map((b) => [`${sec.id} ${b.action}`, b] as const),
      ),
    );
  const base = $derived(index(everyones));
  const used = $derived(store.deviceRaw ? index(store.effectiveSettings?.raw ?? {}) : base);
  /** The keys of `b` in section `id` are this device's own: in the draft, under This device; as saved, under Everyone. */
  const mine = (id: KeySection, b: Binding) =>
    device ? keysOf(b) !== keysOf(base.get(`${id} ${b.action}`)) : keysOf(used.get(`${id} ${b.action}`)) !== keysOf(b);
  /** Under This device: an action's keys as everyone has them again. */
  function putBack(id: KeySection, b: Binding) {
    const was = base.get(`${id} ${b.action}`);
    change(id, b.action, was?.keys ?? null);
  }

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

  // Adding a key to a section (`KeyAddForm`); `addN` starts the form afresh each time Add is pressed.
  let adding = $state<KeySection | null>(null);
  let addN = $state(0);
  function openAdd(section: KeySection) {
    adding = section;
    addN++;
  }

  async function apply() {
    busy = true;
    error = null;
    done = false;
    try {
      if (device) {
        const id = await ensureDeviceId();
        if (!id) return;
        await store.saveDevice(
          withDeviceKeys(store.deviceRaw ?? {}, everyones, $state.snapshot(draft) as Settings),
          id,
        );
      } else await store.saveKeys($state.snapshot(draft) as Settings);
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
        {#key addN}
          <KeyAddForm
            actions={s.actions}
            {keys}
            onchange={(runs, list, group) => change(s.id, runs, list, group)}
            onclose={() => (adding = null)}
          />
        {/key}
      {/if}

      {#if rows.length}
        <table class="keys">
          <tbody>
            {#each rows as b (b.action)}
              <tr class:default={b.keys === null} class:mine={device && mine(s.id, b)}>
                <td class="k">
                  {#if b.keys === null}{@render keys(b.defaults, true)}
                  {:else if b.keys.length}{@render keys(b.keys, false)}
                  {:else}<span class="dim">no key</span>{/if}
                </td>
                <td>
                  <code>{b.command || b.action}</code>
                  {#if b.command && b.command !== b.action}<span class="dim">{b.action}</span>{/if}
                  {#if b.about}<span class="dim">· {b.about}</span>{/if}
                  {#if mine(s.id, b)}
                    {#if device}<span class="own">this device's</span>
                    {:else}<a
                        class="own"
                        href="/settings/device/keys"
                        onclick={(e) => (e.preventDefault(), (store.settingsDevice = true))}>this device uses its own</a
                      >{/if}
                  {/if}
                  {#if b.keys !== null && showDefaults && b.defaults.length}
                    <span class="dim">· default {@render keys(b.defaults, true)}</span>
                  {/if}
                </td>
                <td class="x">
                  {#if device && mine(s.id, b)}
                    <button
                      class="icon"
                      title="Use everyone's keys for it"
                      aria-label="Use everyone's keys for {b.action}"
                      onclick={() => putBack(s.id, b)}><Undo2 size={14} /></button
                    >
                  {:else if b.keys !== null}
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
  /* Keys this device has of its own (spec 0027): the row framed. */
  tr.mine {
    outline: 1px solid var(--accent);
    outline-offset: -1px;
    background: color-mix(in srgb, var(--accent) 7%, transparent);
  }
  .own {
    margin-left: 6px;
    font-size: 11px;
    color: var(--accent);
  }
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
  .icon {
    padding: 2px 4px;
    line-height: 0;
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
