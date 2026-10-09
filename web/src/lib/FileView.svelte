<script lang="ts">
  import { Download } from './icons';
  import type { FileResult } from './types';

  /**
   * A file the command made (`export`): offered as a download rather than printed, since it can be thousands
   * of lines. The first few lines are there to look at.
   */
  let { result }: { result: FileResult } = $props();

  const PREVIEW = 12;
  const lines = $derived(result.text.split('\n'));
  const size = $derived(new Blob([result.text]).size);
  const sizeText = $derived(
    size < 1024
      ? `${size} B`
      : size < 1024 * 1024
        ? `${(size / 1024).toFixed(1)} KB`
        : `${(size / 1024 / 1024).toFixed(1)} MB`,
  );

  function download() {
    const url = URL.createObjectURL(new Blob([result.text], { type: result.mime }));
    const a = document.createElement('a');
    a.href = url;
    a.download = result.name;
    document.body.append(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(url);
  }
</script>

<div class="file">
  <p>
    Exported {result.count}
    {result.count === 1 ? 'task' : 'tasks'} ({sizeText}).
    <button class="primary" onclick={download}><Download size={15} /> Download {result.name}</button>
  </p>
  {#if result.count > 0}
    <details>
      <summary>Preview</summary>
      <pre class="mono">{lines.slice(0, PREVIEW).join('\n')}{lines.length > PREVIEW ? '\n…' : ''}</pre>
    </details>
  {/if}
</div>

<style>
  .file p {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
    margin: 4px 0;
  }
  .file button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  summary {
    color: var(--dim);
    font-size: 13px;
    cursor: pointer;
  }
  pre {
    margin: 4px 0;
    white-space: pre;
    overflow-x: auto;
    font-size: 12px;
  }
</style>
