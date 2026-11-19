<script lang="ts">
  import { TriangleAlert } from './icons';
  import type { TaskrcResponse } from './types';

  let {
    problems,
    error,
    note,
    skipped,
    result,
  }: {
    /** The lines of the file shown that saving would change or refuse. */
    problems: { line: number; message: string }[];
    error: string | null;
    note: string | null;
    /** Files an upload left out. */
    skipped: string[];
    /** What the last save kept, commented out and removed. */
    result: TaskrcResponse | null;
  } = $props();
</script>

<div class="below">
  {#each problems as p, i (i)}<p class="warn"><TriangleAlert size={14} /> Line {p.line}: {p.message}</p>{/each}
  {#if error}<p class="err" role="alert">{error}</p>{/if}
  {#if note}<p class="dim" role="status">{note}</p>{/if}
  {#if skipped.length}
    <p class="warn" role="status">
      <TriangleAlert size={14} /> Left out: <span class="mono">{skipped.join(', ')}</span>
    </p>
  {/if}
  {#if result}
    <div class="result" role="status">
      <p class="ok">
        Saved {result.udas} UDA{result.udas === 1 ? '' : 's'}, {result.reports} report{result.reports === 1 ? '' : 's'},
        {result.contexts} context{result.contexts === 1 ? '' : 's'}.
      </p>
      {#if result.blocked.length}
        <p>
          <strong>Removed for safety ({result.blocked.length}):</strong>
          <span class="mono dim">{result.blocked.join(', ')}</span>
        </p>
      {/if}
      {#if result.commented?.length}
        <details open>
          <summary class="dim"
            >{result.commented.length} line{result.commented.length === 1 ? '' : 's'} commented out</summary
          >
          {#each result.commented as c, i (i)}
            <p class="dim small">
              <span class="mono">{c.file ? `${c.file}:` : 'line '}{c.line}</span>
              {c.message}
            </p>
          {/each}
        </details>
      {/if}
      {#each result.warnings as w, _i (_i)}<p class="warn"><TriangleAlert size={14} /> {w}</p>{/each}
      {#if result.ignored.length}
        <details>
          <summary class="dim">{result.ignored.length} other settings not used</summary>
          <p class="mono dim">{result.ignored.join(', ')}</p>
        </details>
      {/if}
    </div>
  {/if}
</div>

<style>
  .below {
    max-height: 30%;
    overflow: auto;
  }
  .below:empty {
    display: none;
  }
  .warn {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--warn);
    margin: 2px 0;
  }
  .result p {
    margin: 4px 0;
  }
  .small {
    font-size: 12.5px;
  }
</style>
