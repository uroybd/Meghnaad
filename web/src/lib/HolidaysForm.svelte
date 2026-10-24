<script lang="ts">
  import { untrack } from 'svelte';
  import { build, fetchYears, locales, pathFor, settingsOf, type Choice, type Fetched } from './holidays';
  import { CalendarDays } from './icons';

  let {
    busy = false,
    included = () => false,
    calendarOn = false,
    again = null,
    onsave,
  }: {
    busy?: boolean;
    /** Whether the main taskrc already includes this file. */
    included?: (path: string) => boolean;
    /** The calendar already shows holidays. */
    calendarOn?: boolean;
    /** The text of a holiday file made here, to update it: its country and regions are the starting point. */
    again?: { path: string; text: string } | null;
    onsave: (choice: Choice) => Promise<void>;
  } = $props();

  const places = locales();
  const year = new Date().getFullYear();
  /** The browser's own language and region, if Holidata has them: `en-US`. */
  const guess = places.find((p) => p.locale === navigator.language)?.locale ?? '';

  let open = $state(false);
  let locale = $state(guess);
  let first = $state(year - 1);
  let last = $state(year + 2);
  let fetched = $state<Fetched | null>(null);
  let chosen = $state<string[]>([]);
  let wantInclude = $state(true);
  let wantCalendar = $state(true);
  let loading = $state(false);
  let error = $state<string | null>(null);

  const built = $derived(fetched && locale ? build(locale, fetched.bodies, chosen) : null);
  const path = $derived(locale ? pathFor(locale) : '');
  const alreadyIncluded = $derived(path !== '' && included(path));

  // Updating a file made here starts from what it was made for.
  $effect(() => {
    if (!again) return;
    const was = settingsOf(again.text);
    if (!was) return;
    untrack(() => {
      open = true;
      locale = was.locale;
      chosen = was.regions;
      fetched = null;
      void find();
    });
  });

  const reset = () => {
    fetched = null;
    error = null;
  };

  async function find() {
    if (!locale || loading) return;
    loading = true;
    error = null;
    try {
      fetched = await fetchYears(locale, Math.min(first, last), Math.max(first, last));
    } catch (e) {
      fetched = null;
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  const toggle = (name: string, on: boolean) => (chosen = on ? [...chosen, name] : chosen.filter((r) => r !== name));

  async function save() {
    if (!built) return;
    await onsave({
      path,
      text: built.text,
      include: wantInclude && !alreadyIncluded,
      calendar: wantCalendar && !calendarOn,
    });
    fetched = null;
  }
</script>

<details class="holidays" bind:open>
  <summary><CalendarDays size={14} /> Add holidays</summary>
  <p class="dim small">
    Public holidays for a country, from <a href="https://holidata.net" target="_blank" rel="noreferrer">Holidata</a>,
    the data behind Taskwarrior's own holiday files. They are saved as a file here and shown in the calendar.
  </p>
  <form
    class="pick"
    onsubmit={(e) => {
      e.preventDefault();
      void find();
    }}
  >
    <label
      >Country
      <select bind:value={locale} onchange={reset}>
        <option value="">choose…</option>
        {#each places as p (p.locale)}<option value={p.locale}>{p.label}</option>{/each}
      </select>
    </label>
    <label
      >From
      <input type="number" min="1900" max="2100" bind:value={first} oninput={reset} />
    </label>
    <label
      >to
      <input type="number" min="1900" max="2100" bind:value={last} oninput={reset} />
    </label>
    <button disabled={!locale || loading || busy}>{loading ? 'Fetching…' : 'Find holidays'}</button>
  </form>

  {#if error}<p class="err" role="alert">{error}</p>{/if}

  {#if built && fetched}
    <p role="status">
      {built.count} holiday{built.count === 1 ? '' : 's'}{#if fetched.missing.length}
        (Holidata has none for {fetched.missing.join(', ')}){/if}.
    </p>
    {#if built.regions.length}
      <fieldset>
        <legend>Also the holidays of</legend>
        {#each built.regions as r (r)}
          <label class="check"
            ><input type="checkbox" checked={chosen.includes(r)} onchange={(e) => toggle(r, e.currentTarget.checked)} />
            {r}</label
          >
        {/each}
      </fieldset>
    {/if}
    {#if !alreadyIncluded}
      <label class="check"><input type="checkbox" bind:checked={wantInclude} /> Include it in <code>taskrc</code></label
      >
    {/if}
    {#if !calendarOn}
      <label class="check"
        ><input type="checkbox" bind:checked={wantCalendar} /> Show holidays in the calendar
        <span class="dim">(<code>calendar.holidays=sparse</code>)</span></label
      >
    {/if}
    <button class="primary" onclick={save} disabled={busy || built.count === 0}>Save <code>{path}</code></button>
    <span class="dim small">replaces that file if it is there; Restore previous undoes it</span>
  {/if}
</details>

<style>
  .holidays {
    margin-top: 12px;
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 6px 10px;
  }
  summary {
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 6px;
    font-weight: 600;
  }
  .pick {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 14px;
  }
  .pick input[type='number'] {
    width: 5.5em;
  }
  fieldset {
    border: 1px solid var(--border);
    border-radius: 6px;
    margin: 8px 0;
    display: flex;
    flex-wrap: wrap;
    gap: 4px 16px;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 4px 0;
  }
  fieldset .check {
    margin: 0;
  }
  .small {
    font-size: 12.5px;
  }
  code {
    font-family: var(--mono);
    background: var(--panel-2);
    border-radius: 4px;
    padding: 0 4px;
    overflow-wrap: anywhere;
  }
</style>
