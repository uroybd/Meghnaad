<script lang="ts">
  import { progressFraction, type Progress } from './appsettings';

  /** A value drawn as a bar between `range.min` (empty) and `range.max` (full), with the value written beside it. */
  let { value, text, range, label }: { value: number; text: string; range: Progress; label: string } = $props();

  const fraction = $derived(progressFraction(value, range));
</script>

<span
  class="progress"
  role="progressbar"
  aria-label={label}
  aria-valuemin={range.min}
  aria-valuemax={range.max}
  aria-valuenow={Math.min(range.max, Math.max(range.min, value))}
  aria-valuetext={text}
>
  <span class="track"><span class="fill" class:full={fraction >= 1} style="width:{fraction * 100}%"></span></span>
  <span class="num">{text}</span>
</span>

<style>
  .progress {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    min-width: 110px;
  }
  .track {
    flex: 1;
    min-width: 60px;
    height: 8px;
    border-radius: 4px;
    background: var(--line);
    overflow: hidden;
  }
  .fill {
    display: block;
    height: 100%;
    border-radius: 4px;
    background: var(--accent);
  }
  .fill.full {
    background: var(--ok, var(--accent));
  }
  .num {
    font-size: 12px;
    color: var(--dim);
    font-variant-numeric: tabular-nums;
  }
</style>
