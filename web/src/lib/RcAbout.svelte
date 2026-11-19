<script lang="ts">
  import { Info } from './icons';

  let open = $state(false);
</script>

<header class="head">
  <h2>taskrc</h2>
  <span class="dim">A copy of your taskrc, as a folder of files, saved with your tasks.</span>
  <button
    class="ghost about"
    aria-expanded={open}
    aria-label="About the taskrc copy"
    title="About the taskrc copy"
    onclick={() => (open = !open)}><Info size={15} /></button
  >
</header>
{#if open}
  <p class="dim about-text">
    UDAs, custom reports, contexts, holidays and journal settings live in your <code>~/.taskrc</code>, not in your
    synced data, so this app keeps a copy: a folder of files, as written. <code>include</code> works as in Taskwarrior,
    with the path written from the file that has it. They are saved with your tasks and stay until you change them. A
    line this app can't use is commented out, not lost.
    <strong>Sync settings and anything that looks like a credential are removed</strong>: they're never stored, logged
    or shown. The colours are set in Appearance.
  </p>
{/if}

<style>
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 4px 10px;
  }
  h2 {
    margin: 0;
    font-size: 18px;
  }
  .about {
    line-height: 0;
    align-self: center;
  }
  .about-text {
    margin: 0;
  }
  code {
    font-family: var(--mono);
    background: var(--panel-2);
    border-radius: 4px;
    padding: 0 4px;
    overflow-wrap: anywhere;
  }
</style>
