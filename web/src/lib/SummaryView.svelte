<script lang="ts">
  import { store } from './store.svelte';
  import type { SummaryResult } from './types';

  /** `summary`: how far along each project is, with its sub-projects indented beneath it. */
  let { result }: { result: SummaryResult } = $props();

  const CELLS = 30; // the bar's width in Taskwarrior, in cells

  // A project's name shows that project's tasks in the Tasks view (and its sub-projects'), in the
  // built-in `list` report, which is always there.
  const show = (project: string) => store.focusReport('list', [project ? `project:${project}` : 'project:']);
</script>

<div class="scroll">
  <table>
    <thead>
      <tr>
        <th>Project</th>
        <th class="num">Remaining</th>
        <th class="num">Avg age</th>
        <th class="num">Complete</th>
        <th class="barcol"><span>0%</span><span>100%</span></th>
      </tr>
    </thead>
    <tbody>
      {#each result.rows as row (row.project)}
        <tr>
          <td class="project" style="padding-left: {row.depth * 1.4}em">
            <button type="button" class="link" title="Show these tasks" onclick={() => show(row.project)}
              >{row.label}</button
            >
          </td>
          <td class="num">{row.remaining}</td>
          <td class="num">{row.avg_age}</td>
          <td class="num">{row.complete}</td>
          <td class="barcol">
            <div
              class="bar"
              role="progressbar"
              aria-label="{row.label}: {row.complete} complete"
              aria-valuemin="0"
              aria-valuemax="100"
              aria-valuenow={Number.parseInt(row.complete)}
            >
              <span style="width: {(row.bar / CELLS) * 100}%"></span>
            </div>
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>
<p class="dim count">{result.rows.length} {result.rows.length === 1 ? 'project' : 'projects'}</p>

<style>
  .scroll {
    overflow-x: auto;
  }
  table {
    border-collapse: collapse;
    width: 100%;
    max-width: 760px;
  }
  th,
  td {
    text-align: left;
    padding: 5px 16px 5px 0;
    white-space: nowrap;
  }
  th {
    color: var(--dim);
    font-weight: 500;
    font-size: 13px;
    border-bottom: 1px solid var(--line);
  }
  tbody td {
    border-bottom: 1px solid color-mix(in srgb, var(--line) 60%, transparent);
  }
  .num {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .project {
    font-weight: 500;
  }
  .link {
    background: none;
    border: 0;
    padding: 0;
    font: inherit;
    color: inherit;
    cursor: pointer;
    text-align: left;
  }
  .link:hover {
    color: var(--accent);
    text-decoration: underline;
  }
  .barcol {
    width: 100%;
    min-width: 150px;
    padding-right: 0;
  }
  th.barcol {
    display: table-cell;
    font-weight: 400;
  }
  th.barcol span:last-child {
    float: right;
  }
  .bar {
    height: 12px;
    border-radius: 6px;
    background: var(--cc-summary-background-bg, var(--panel-2));
    border: 1px solid var(--line);
    overflow: hidden;
  }
  .bar span {
    display: block;
    height: 100%;
    background: var(--cc-summary-bar-bg, var(--ok));
    border-radius: 6px 0 0 6px;
  }
  .count {
    margin: 8px 0;
  }
  @media (max-width: 600px) {
    th,
    td {
      padding-right: 8px;
      font-size: 13px;
    }
    th {
      font-size: 12px;
    }
    .barcol {
      min-width: 72px;
    }
  }
</style>
