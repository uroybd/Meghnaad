<script lang="ts">
  import { moveSeries, type Section, type WrittenGroup } from './boards';
  import { Plus } from './icons';
  import SectionItem from './SectionItem.svelte';

  /** The groups a chart's board writes, as Chart.js has labels: each a label, a filter, and a colour of its own. */
  let { section, set }: { section: Section; set: (patch: Partial<Section>) => void } = $props();

  const MAX_GROUPS = 24;
  const written = $derived(section.by?.groups ?? []);
  const setGroups = (list: WrittenGroup[]) => set({ by: { groups: list } });
  const patchGroup = (i: number, patch: Partial<WrittenGroup>) =>
    setGroups(written.map((g, j) => (j === i ? { ...g, ...patch } : g)));
  const addGroup = () => {
    const taken = new Set(written.map((g) => g.label.trim()));
    let n = written.length + 1;
    while (taken.has(`Group ${n}`)) n++;
    setGroups([...written, { label: `Group ${n}` }]);
  };
</script>

<fieldset class="wide series">
  <legend>Groups</legend>
  <p class="dim hint">
    Each group is a slice of a pie, a bar or a point: the tasks its filter picks (added to the filters of the board, the
    chart and each series, unless it leaves them out), in this order.
  </p>
  {#each written as g, i (i)}
    <SectionItem
      what="group"
      {i}
      count={written.length}
      item={g}
      filterId="group-filter-{section.id}-{i}"
      placeholder="Which tasks: status:completed outcome:success"
      earlier="Move earlier"
      later="Move later"
      removable={written.length !== 1}
      onpatch={(patch) => patchGroup(i, patch)}
      onmove={(by) => setGroups(moveSeries(written, i, by))}
      onremove={() => setGroups(written.filter((_, j) => j !== i))}
    />
  {/each}
  <div class="row-series">
    <button onclick={addGroup} disabled={written.length >= MAX_GROUPS}><Plus size={14} /> Add group</button>
  </div>
</fieldset>

<style>
  .wide {
    grid-column: 1 / -1;
  }
  .series {
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 8px 10px;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .series legend {
    padding: 0 4px;
    font-size: 12px;
    color: var(--dim);
  }
  .hint {
    margin: 0;
  }
  .row-series {
    display: flex;
    align-items: center;
    gap: 6px;
  }
</style>
