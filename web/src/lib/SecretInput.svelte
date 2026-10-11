<script lang="ts">
  import { Eye, EyeOff } from './icons';

  /** A field for a secret: dots to begin with, and a button to see what was typed (to check a long key). */
  let {
    value = $bindable(''),
    id,
    label,
    placeholder,
  }: {
    value?: string;
    id?: string;
    /** The field's name, when no <label for> names it. */
    label?: string;
    placeholder?: string;
  } = $props();

  let shown = $state(false);
</script>

<span class="secret">
  <input
    bind:value
    {id}
    {placeholder}
    aria-label={label}
    type={shown ? 'text' : 'password'}
    autocomplete="off"
    autocapitalize="off"
    spellcheck="false"
  />
  <button
    type="button"
    class="ghost eye"
    aria-label={shown ? 'Hide what is typed' : 'Show what is typed'}
    aria-pressed={shown}
    onclick={() => (shown = !shown)}
  >
    {#if shown}<EyeOff size={16} />{:else}<Eye size={16} />{/if}
  </button>
</span>

<style>
  .secret {
    position: relative;
    display: flex;
    align-items: center;
    flex: 1 1 12rem;
    min-width: 0;
  }
  input {
    width: 100%;
    padding-right: 44px;
  }
  .eye {
    position: absolute;
    right: 2px;
    top: 50%;
    transform: translateY(-50%);
    min-width: 40px;
    min-height: 40px;
    padding: 0;
    border-color: transparent;
    background: transparent;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    line-height: 0;
    color: var(--dim);
  }
</style>
