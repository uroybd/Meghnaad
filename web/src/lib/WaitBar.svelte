<script lang="ts">
  import { waiting } from './waiting.svelte';
</script>

<!-- A thin bar while the page waits on a request or a command; shown only after a moment, so quick ones don't flash. -->
{#if waiting.count > 0}<div class="bar" aria-hidden="true"></div>{/if}

<style>
  .bar {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    height: 2px;
    z-index: 100;
    overflow: hidden;
    opacity: 0;
    animation: show 0s linear 0.2s forwards;
  }
  .bar::after {
    content: '';
    position: absolute;
    inset: 0;
    width: 30%;
    background: var(--accent);
    animation: slide 1.1s ease-in-out infinite;
  }
  @keyframes show {
    to {
      opacity: 1;
    }
  }
  @keyframes slide {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(340%);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .bar::after {
      width: 100%;
      animation: none;
      opacity: 0.6;
    }
  }
</style>
