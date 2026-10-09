<script lang="ts">
  /** A task's state as a coloured pill: each state has its own colour. */
  export type Kind = 'pending' | 'completed' | 'deleted' | 'recurring' | 'waiting' | 'blocked' | 'blocking';
  let { kind, label = kind }: { kind: Kind; label?: string } = $props();
</script>

<span class="pill {kind}">{label}</span>

<style>
  .pill {
    --c: var(--text);
    display: inline-block;
    padding: 0 9px;
    border-radius: 999px;
    font-size: 12.5px;
    line-height: 21px;
    color: var(--c);
    background: color-mix(in srgb, var(--c) 13%, transparent);
    border: 1px solid color-mix(in srgb, var(--c) 35%, transparent);
  }
  .pending {
    --c: var(--accent);
  }
  .completed {
    --c: var(--ok);
  }
  .deleted {
    --c: var(--err);
  }
  .recurring {
    --c: hsl(265 var(--seg-s) var(--seg-l));
  }
  .waiting {
    --c: var(--warn);
  }
  .blocked {
    --c: hsl(25 var(--seg-s) var(--seg-l));
  }
  .blocking {
    --c: hsl(180 var(--seg-s) var(--seg-l));
  }
</style>
