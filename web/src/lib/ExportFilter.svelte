<script lang="ts">
  import { runCli } from './api';
  import { splitWords } from './cmdline';
  import FilterChips from './FilterChips.svelte';
  import FilterInput from './FilterInput.svelte';
  import { store } from './store.svelte';
  import type { Row } from './types';

  /**
   * The filter box of the Projects and Tags pages, and the tasks it selects (`task <filter> export`, any status).
   * Like Summary and Burndown the filter is the user's: they start with `status:pending`, and taking that chip
   * away brings finished tasks in, so a project or tag stays listed after its last task is done.
   */
  let {
    filter = $bindable(''),
    onresult,
    id,
    label,
    placeholder,
  }: {
    filter: string;
    /** Called after each answer: the tasks, or why there are none (the last good tasks are then kept by the caller). */
    onresult: (r: { rows: Row[] | null; error: string | null }) => void;
    id: string;
    label: string;
    placeholder?: string;
  } = $props();

  let seq = 0;
  async function load(f: string) {
    const mine = ++seq;
    try {
      const r = (await runCli({ args: [...splitWords(f), '_rows'] })).result;
      if (mine !== seq) return; // a newer filter was asked for meanwhile
      if (r.kind === 'error') throw new Error(r.message);
      onresult({ rows: r.kind === 'json' ? (r.value as Row[]) : null, error: null });
    } catch (e) {
      if (mine === seq) onresult({ rows: null, error: e instanceof Error ? e.message : String(e) });
    }
  }

  // Ask again when the filter changes and when a task changes anywhere (done, edit, add, console...). Only typing
  // waits a moment; opening the page does not.
  let lastFilter: string | null = null;
  $effect(() => {
    const f = filter;
    void store.rev;
    const typing = lastFilter !== null && f !== lastFilter;
    const t = setTimeout(
      () => {
        lastFilter = f;
        void load(f);
      },
      typing ? 250 : 0,
    );
    return () => clearTimeout(t);
  });
</script>

<section class="bar" aria-label={label}>
  <FilterInput bind:value={filter} {id} {placeholder} autofocus />
  <FilterChips bind:value={filter} />
</section>

<style>
  .bar {
    display: grid;
    gap: 10px;
    margin-bottom: 14px;
  }
</style>
