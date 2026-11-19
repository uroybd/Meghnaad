<script lang="ts">
  import { Link, Upload } from './icons';

  let {
    busy,
    link = $bindable(),
    replace = $bindable(),
    onlink,
    onupload,
  }: {
    busy: boolean;
    /** The link typed, kept by the panel while the drawer is closed. */
    link: string;
    /** A folder replaces what is there, rather than adding to it. */
    replace: boolean;
    onlink: () => void;
    onupload: (e: Event) => void;
  } = $props();
</script>

<form
  class="link"
  onsubmit={(e) => {
    e.preventDefault();
    onlink();
  }}
>
  <label for="rc-link"><Link size={14} /> From a link</label>
  <input
    id="rc-link"
    type="url"
    bind:value={link}
    placeholder="https://raw.githubusercontent.com/you/dotfiles/main/.taskrc"
    spellcheck="false"
    autocomplete="off"
  />
  <button disabled={busy || !link.trim()}>Import</button>
  <p class="dim small">
    Fetched and saved as the main taskrc straight away (Restore previous undoes it). GitHub and GitLab file pages work
    too. A public link; keep it free of passwords. Files it includes are not fetched: add them with + New.
  </p>
</form>
<hr />
<div class="folder-import">
  <p><Upload size={14} /> A folder, as it is</p>
  <label class="check"
    ><input type="checkbox" bind:checked={replace} /> Replace what is here (needs a <code>taskrc</code>)</label
  >
  <input type="file" multiple webkitdirectory onchange={onupload} disabled={busy} aria-label="Folder to import" />
  <p class="dim small">Without Replace, the files are added to what is here, and same names are overwritten.</p>
</div>

<style>
  .link {
    display: grid;
    gap: 8px;
  }
  .link label,
  .folder-import p:first-child {
    display: flex;
    align-items: center;
    gap: 6px;
    font-weight: 600;
    margin: 0;
  }
  .folder-import {
    display: grid;
    gap: 8px;
  }
  hr {
    border: 0;
    border-top: 1px solid var(--border);
    margin: 16px 0;
  }
  .check {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .small {
    font-size: 12.5px;
  }
  code {
    font-family: var(--mono);
    background: var(--panel-2);
    border-radius: 4px;
    padding: 0 4px;
    overflow-wrap: anywhere;
  }
</style>
