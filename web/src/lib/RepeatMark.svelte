<script lang="ts">
  // The mark after a repeating task's title, in reports and on cards: `repeat` for a recurring task (its template or
  // one of its instances), `iteration-cw` for an iterative one (done, it makes the next). The words are for a tooltip
  // and a screen reader.
  import { IterationCw, Repeat } from './icons';
  import { describeRecur } from './recurrence';
  import type { Row } from './types';

  let { row }: { row: Pick<Row, 'status' | 'recur' | 'iter' | 'iter_count'> } = $props();

  const what = $derived(
    row.recur
      ? row.status === 'recurring'
        ? `Recurring task: repeats ${describeRecur(row.recur)}`
        : `Repeats ${describeRecur(row.recur)}`
      : row.status === 'iterative' && row.iter
        ? `Iterative: done, it makes the next (${row.iter})${row.iter_count ? `; number ${row.iter_count}` : ''}`
        : '',
  );
</script>

{#if what}
  <span class="repeat-mark" title={what} data-kind={row.recur ? 'recurring' : 'iterative'}>
    {#if row.recur}<Repeat size={12} />{:else}<IterationCw size={12} />{/if}<span class="sr-only">{what}</span>
  </span>
{/if}

<style>
  .repeat-mark {
    display: inline-flex;
    margin-left: 6px;
    color: var(--dim);
    vertical-align: -1px;
  }
</style>
