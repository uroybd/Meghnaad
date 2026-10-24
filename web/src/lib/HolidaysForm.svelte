<script lang="ts">
  import { untrack } from 'svelte';
  import { fetchCalendar } from './api';
  import {
    buildCalendar,
    buildHolidata,
    buildNager,
    calendarLink,
    fetchHolidata,
    fetchNager,
    holidataPlaces,
    nagerPlaces,
    settingsOf,
    type Built,
    type Choice,
    type Fetched,
    type Place,
    type Source,
  } from './holidays';
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
    /** The text of a holiday file made here, to update it: what it was made for is the starting point. */
    again?: { path: string; text: string } | null;
    onsave: (choice: Choice) => Promise<void>;
  } = $props();

  const holidata = holidataPlaces();
  const year = new Date().getFullYear();
  /** The browser's own language and region, if Holidata has them: `en-US`. */
  const guess = holidata.find((p) => p.id === navigator.language)?.id ?? '';
  /** How many holidays to list before "and N more". */
  const SHOWN = 60;

  let open = $state(false);
  let source = $state<Source>('holidata');
  let place = $state<Record<'holidata' | 'nager', string>>({ holidata: guess, nager: '' });
  let nager = $state<Place[] | null>(null);
  let link = $state('');
  let calName = $state('');
  let first = $state(year - 1);
  let last = $state(year + 2);
  let fetched = $state<Fetched | null>(null);
  /** A calendar's text, and the link it came from. */
  let calendar = $state<{ text: string; link: string } | null>(null);
  let chosen = $state<string[]>([]);
  /** The names of a calendar to leave out; `null` is the calendar's observances. */
  let skip = $state<string[] | null>(null);
  let wantInclude = $state(true);
  let wantCalendar = $state(true);
  let loading = $state(false);
  let error = $state<string | null>(null);

  const sel = $derived(source === 'ics' ? '' : place[source]);
  const built = $derived.by((): Built | null => {
    if (source === 'ics') {
      if (!calendar) return null;
      const lo = Math.min(first, last);
      const hi = Math.max(first, last);
      return buildCalendar(calName.trim(), calendar.text, lo, hi, skip, calendar.link);
    }
    if (!fetched || !sel) return null;
    return (source === 'nager' ? buildNager : buildHolidata)(sel, fetched.bodies, chosen);
  });
  const summary = $derived.by(() => {
    if (!built) return '';
    const n = built.count;
    const parts = [`${n} holiday${n === 1 ? '' : 's'}`];
    if (built.title) parts.push(`in ${built.title}`);
    if (fetched?.missing.length) parts.push(`(none for ${fetched.missing.join(', ')})`);
    if (built.skipped) parts.push(`(${built.skipped} entr${built.skipped === 1 ? 'y' : 'ies'} could not be used)`);
    return `${parts.join(' ')}.`;
  });
  const alreadyIncluded = $derived(built ? included(built.path) : false);

  // Nager.Date's countries are asked for once, when it is first chosen.
  $effect(() => {
    if (source !== 'nager' || nager || !open) return;
    untrack(() => {
      void (async () => {
        try {
          nager = await nagerPlaces();
        } catch (e) {
          error = e instanceof Error ? e.message : String(e);
        }
      })();
    });
  });

  // Updating a file made here starts from what it was made for.
  $effect(() => {
    if (!again) return;
    const was = settingsOf(again.text);
    if (!was) return;
    untrack(() => {
      open = true;
      source = was.source;
      chosen = was.regions;
      skip = was.source === 'ics' ? was.skip : null;
      if (was.source === 'ics') {
        calName = was.key;
        link = was.link ?? '';
      } else {
        place[was.source] = was.key;
      }
      reset();
      if (was.source !== 'ics' || was.link) void find();
    });
  });

  function reset() {
    fetched = null;
    calendar = null;
    error = null;
  }

  const ready = $derived(source === 'ics' ? link.trim() !== '' : sel !== '');

  async function find() {
    if (!ready || loading) return;
    loading = true;
    error = null;
    try {
      const lo = Math.min(first, last);
      const hi = Math.max(first, last);
      if (source === 'ics') {
        const checked = calendarLink(link);
        if ('problem' in checked) throw new Error(checked.problem);
        const text = await fetchCalendar(checked.link);
        if (!/^\s*BEGIN:VCALENDAR/i.test(text)) throw new Error('That link is not a calendar (.ics) file.');
        calendar = { text, link: checked.link };
      } else {
        fetched = await (source === 'nager' ? fetchNager : fetchHolidata)(sel, lo, hi);
      }
    } catch (e) {
      fetched = null;
      calendar = null;
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  const toggle = (name: string, on: boolean) => (chosen = on ? [...chosen, name] : chosen.filter((r) => r !== name));
  const toggleName = (name: string, keep: boolean) => {
    const left = (built?.names ?? []).filter((n) => n.skip).map((n) => n.name);
    skip = keep ? left.filter((n) => n !== name) : [...left, name];
  };

  async function save() {
    if (!built) return;
    await onsave({
      path: built.path,
      text: built.text,
      include: wantInclude && !alreadyIncluded,
      calendar: wantCalendar && !calendarOn,
    });
    reset();
  }

  const day = (d: string) => `${d.slice(0, 4)}-${d.slice(4, 6)}-${d.slice(6)}`;
  const sources: { id: Source; label: string }[] = [
    { id: 'holidata', label: 'Holidata (42 countries)' },
    { id: 'nager', label: 'Nager.Date (200+ countries)' },
    { id: 'ics', label: 'Calendar link (.ics)' },
  ];
</script>

<details class="holidays" bind:open>
  <summary><CalendarDays size={14} /> Add holidays</summary>
  <p class="dim small">
    Public holidays for a country, saved as a file here and shown in the calendar. Holidata is the data behind
    Taskwarrior's own holiday files. Nager.Date covers more countries, but for some it lists only the days that fall on
    the same date every year. A calendar link (a public <code>.ics</code> feed, such as Google's holiday calendars) covers
    whatever the calendar does.
  </p>
  <div class="sources" role="radiogroup" aria-label="Where the holidays come from">
    {#each sources as s (s.id)}
      <label class="check"
        ><input type="radio" name="holiday-source" value={s.id} bind:group={source} onchange={reset} /> {s.label}</label
      >
    {/each}
  </div>
  <form
    class="pick"
    onsubmit={(e) => {
      e.preventDefault();
      void find();
    }}
  >
    {#if source === 'holidata'}
      <label
        >Country
        <select bind:value={place.holidata} onchange={reset}>
          <option value="">choose…</option>
          {#each holidata as p (p.id)}<option value={p.id}>{p.label}</option>{/each}
        </select>
      </label>
    {:else if source === 'nager'}
      <label
        >Country
        <select bind:value={place.nager} onchange={reset} disabled={!nager}>
          <option value="">{nager ? 'choose…' : 'loading…'}</option>
          {#each nager ?? [] as p (p.id)}<option value={p.id}>{p.label}</option>{/each}
        </select>
      </label>
    {:else}
      <label class="wide"
        >Link to the calendar
        <input
          type="url"
          bind:value={link}
          oninput={reset}
          placeholder="https://calendar.google.com/calendar/ical/en.bd%23holiday%40group.v.calendar.google.com/public/basic.ics"
          spellcheck="false"
          autocomplete="off"
        />
      </label>
    {/if}
    <label
      >From
      <input
        type="number"
        min="1900"
        max="2100"
        bind:value={first}
        oninput={() => (source === 'ics' ? undefined : reset())}
      />
    </label>
    <label
      >to
      <input
        type="number"
        min="1900"
        max="2100"
        bind:value={last}
        oninput={() => (source === 'ics' ? undefined : reset())}
      />
    </label>
    <button disabled={!ready || loading || busy}>{loading ? 'Fetching…' : 'Find holidays'}</button>
  </form>
  {#if source === 'ics'}
    <p class="dim small">
      A public link, with no password in it. The Worker fetches it for you; nothing is saved until you choose Save.
    </p>
  {/if}

  {#if error}<p class="err" role="alert">{error}</p>{/if}

  {#if built && (fetched || calendar)}
    <p role="status">{summary}</p>
    {#if source === 'nager'}
      <p class="dim small">
        Look the list over: Nager.Date may not have the holidays that move with the moon (Eid, Diwali) for every
        country.
      </p>
    {/if}
    {#if source === 'ics' && built.names.length}
      <fieldset class="names">
        <legend>Keep</legend>
        {#each built.names as n (n.name)}
          <label class="check"
            ><input type="checkbox" checked={!n.skip} onchange={(e) => toggleName(n.name, e.currentTarget.checked)} />
            {n.name}
            <span class="dim">{n.count > 1 ? `×${n.count}` : ''}{n.kind ? ` ${n.kind}` : ''}</span></label
          >
        {/each}
      </fieldset>
    {/if}
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
    <ul class="entries" aria-label="The holidays found">
      {#each built.entries.slice(0, SHOWN) as e, i (i)}
        <li><span class="mono">{day(e.date)}{e.end ? ` – ${day(e.end)}` : ''}</span> {e.name}</li>
      {/each}
      {#if built.entries.length > SHOWN}<li class="dim">and {built.entries.length - SHOWN} more</li>{/if}
    </ul>
    {#if source === 'ics'}
      <label class="wide"
        >Name of the file
        <input bind:value={calName} placeholder={built.key} spellcheck="false" autocomplete="off" />
      </label>
    {/if}
    {#if !alreadyIncluded}
      <label class="check"><input type="checkbox" bind:checked={wantInclude} /> Include it in <code>taskrc</code></label
      >
    {/if}
    {#if !calendarOn}
      <label class="check"
        ><input type="checkbox" bind:checked={wantCalendar} /> Show holidays in the calendar
        <span class="dim">(<code>calendar.holidays=full</code>)</span></label
      >
    {/if}
    <button class="primary" onclick={save} disabled={busy || built.count === 0}>Save <code>{built.path}</code></button>
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
  .sources {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 16px;
    margin: 6px 0;
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
  .wide {
    flex: 1 1 100%;
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 4px 0;
  }
  fieldset {
    border: 1px solid var(--border);
    border-radius: 6px;
    margin: 8px 0;
    display: flex;
    flex-wrap: wrap;
    gap: 4px 16px;
  }
  .names {
    max-height: 14em;
    overflow: auto;
  }
  .entries {
    list-style: none;
    margin: 8px 0;
    padding: 6px 8px;
    border: 1px solid var(--border);
    border-radius: 6px;
    max-height: 14em;
    overflow: auto;
    font-size: 13px;
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
