<script lang="ts">
  import { tick } from 'svelte';
  import { ICON_NAME } from './appsettings';
  import CommandIcon from './CommandIcon.svelte';
  import { Search, X } from './icons';
  import { CDN } from './lucide';
  import { lucide } from './lucideLib.svelte';

  /** Pick a Lucide icon by looking at it, or by its name. An empty value is no icon. */
  let { value = $bindable(''), label = 'Icon' }: { value?: string; label?: string } = $props();

  /** How many are drawn at once: each one is fetched when it appears, so a broad search is cut short. */
  const SHOWN = 48;

  let open = $state(false);
  let query = $state('');
  let search: HTMLInputElement | undefined = $state();

  const matches = $derived(lucide.search(query));
  const shown = $derived(matches.slice(0, SHOWN));
  const total = $derived(Object.keys(lucide.catalog ?? {}).length);
  /** What was typed, when it is an icon's name and the list of icons could not be had to check it against. */
  const typed = $derived(query.trim().toLowerCase());
  const usable = $derived(ICON_NAME.test(typed) && !lucide.catalog);

  async function toggle() {
    open = !open;
    if (open) {
      void lucide.loadCatalog();
      query = '';
      await tick();
      search?.focus();
    }
  }

  function pick(name: string) {
    value = name;
    open = false;
  }
</script>

<span class="picker">
  <button
    type="button"
    class="current"
    aria-haspopup="dialog"
    aria-expanded={open}
    aria-label={value ? `${label}: ${value}` : `${label}: none, choose one`}
    title={value || 'Choose an icon'}
    onclick={toggle}
  >
    <CommandIcon name={value || undefined} size={16} />
  </button>
  {#if open}
    <div
      class="pop"
      role="dialog"
      tabindex="-1"
      aria-label="Choose an icon"
      onkeydown={(e) => e.key === 'Escape' && ((open = false), e.stopPropagation())}
    >
      <label class="find"
        ><Search size={14} /><span class="sr-only">Search icons</span>
        <input
          bind:this={search}
          bind:value={query}
          onkeydown={(e) => e.key === 'Enter' && usable && (e.preventDefault(), pick(typed))}
          placeholder={total ? `Search ${total} icons` : 'Search icons, or type a name'}
          spellcheck="false"
          autocomplete="off"
        />
      </label>
      <div class="grid" role="listbox" aria-label="Icons">
        {#each shown as name (name)}
          <button
            type="button"
            role="option"
            aria-selected={name === value}
            class:on={name === value}
            title={name}
            aria-label={name}
            onclick={() => pick(name)}><CommandIcon {name} size={18} /></button
          >
        {/each}
      </div>
      <p class="dim foot">
        {#if lucide.catalogFailed}
          The list of icons could not be fetched from {new URL(CDN).host}. Type an icon's name to use it.
        {:else if !lucide.catalog}
          Fetching the list of icons…
        {:else if !matches.length}
          Nothing is called that.
        {:else if matches.length > shown.length}
          {shown.length} of {matches.length}: type more to narrow it.
        {:else}
          {matches.length} found.
        {/if}
        {#if usable}<button type="button" class="ghost use" onclick={() => pick(typed)}>Use “{typed}”</button>{/if}
        {#if value}<button type="button" class="ghost none" onclick={() => pick('')}><X size={12} /> No icon</button
          >{/if}
      </p>
    </div>
  {/if}
</span>
{#if value && lucide.missing[value]}<span class="warn small">No Lucide icon is called “{value}”.</span>{/if}

<style>
  .picker {
    position: relative;
    display: inline-flex;
  }
  .current {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 38px;
    height: 36px;
    padding: 0;
  }
  .pop {
    position: absolute;
    z-index: 30;
    top: calc(100% + 4px);
    /* Under the button and out to the left: it sits at the right end of its row, and there is no room that way. */
    right: 0;
    width: min(360px, 86vw);
    padding: 8px;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.22);
  }
  .find {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .find input {
    flex: 1;
    min-width: 0;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(40px, 1fr));
    gap: 4px;
    margin: 8px 0 4px;
    max-height: 240px;
    overflow: auto;
  }
  .grid button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    height: 38px;
    padding: 0;
    border-color: transparent;
    background: transparent;
  }
  .grid button:hover {
    background: var(--panel-2);
  }
  .grid button.on {
    border-color: var(--accent);
    background: var(--panel-2);
  }
  .foot {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 8px;
    margin: 4px 0 0;
    font-size: 12.5px;
  }
  .none {
    margin-left: auto;
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .warn {
    color: var(--warn);
  }
  .small {
    font-size: 12.5px;
  }
  /* On a phone the picker is in the middle of its row: let the panel take the row's whole width instead. */
  @media (max-width: 760px) {
    .picker {
      position: static;
    }
    .pop {
      left: 0;
      right: 0;
      width: auto;
    }
  }
</style>
