<script lang="ts">
  import { onMount } from 'svelte';
  import {
    ApiError,
    checkHooks,
    deleteSecret,
    getHooks,
    getSecretNames,
    putHooks,
    putSecret,
    type HookCheck,
  } from './api';
  import type { HookEditor } from './hookEditor';
  import { HOOKS_EXAMPLE } from './hooksExample';
  import { TriangleAlert } from './icons';
  import { store } from './store.svelte';

  /** There are edits not yet saved (the tab shows a dot). */
  let { ondirty }: { ondirty?: (dirty: boolean) => void } = $props();

  let host = $state<HTMLDivElement>();
  let editor: HookEditor | undefined;
  /** The script as saved, and as it is in the editor now. */
  let saved = $state('');
  let text = $state('');
  let loading = $state(true);
  let loadFailed = $state<string | null>(null);
  let result = $state<HookCheck | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let done = $state(false);

  // Secrets: kept from the app, sealed in the bucket. Only their names are ever shown.
  let secrets = $state<string[]>([]);
  let secretName = $state('');
  let secretValue = $state('');
  let secretBusy = $state(false);
  let secretError = $state<string | null>(null);
  const secretNameOk = $derived(/^[A-Za-z][A-Za-z0-9_]{0,63}$/.test(secretName));

  async function loadSecrets() {
    try {
      secrets = await getSecretNames();
    } catch {
      // The editor works without them; setting one says why it could not.
    }
  }

  async function keepSecret() {
    if (!secretNameOk || !secretValue || secretBusy) return;
    secretBusy = true;
    secretError = null;
    try {
      await putSecret(secretName, secretValue);
      store.notify(`Secret ${secretName} saved. Read it in a hook with secret("${secretName}").`, 'ok');
      secretName = '';
      secretValue = '';
      await loadSecrets();
    } catch (e) {
      secretError = e instanceof ApiError ? e.message : 'The secret could not be saved.';
    } finally {
      secretBusy = false;
    }
  }

  async function removeSecret(name: string) {
    secretError = null;
    try {
      await deleteSecret(name);
      await loadSecrets();
    } catch (e) {
      secretError = e instanceof ApiError ? e.message : 'The secret could not be removed.';
    }
  }

  const edited = $derived(text !== saved);
  $effect(() => {
    ondirty?.(edited);
  });

  let timer: ReturnType<typeof setTimeout> | undefined;
  /** Ask the server (its Lua is the one that will run the script) whether this one is all right, a moment after typing. */
  function recheck() {
    clearTimeout(timer);
    timer = setTimeout(async () => {
      const asked = text;
      try {
        const r = await checkHooks(asked);
        if (asked !== text) return;
        result = r;
        editor?.mark(r.ok ? [] : [{ line: r.line, message: r.message }]);
      } catch {
        // Offline: nothing to say about the script; saving says why it could not.
      }
    }, 400);
  }

  onMount(() => {
    let alive = true;
    void loadSecrets();
    (async () => {
      try {
        saved = text = await getHooks();
        const { createHookEditor } = await import('./hookEditor');
        if (!alive || !host) return;
        editor = createHookEditor(host, text, (t) => {
          text = t;
          done = false;
          error = null;
          recheck();
        });
        // Ctrl/Cmd+S saves, from inside the editor.
        host.addEventListener('keydown', onkeydown);
        if (text.trim()) recheck();
      } catch (e) {
        loadFailed = e instanceof Error ? e.message : String(e);
      } finally {
        loading = false;
      }
    })();
    return () => {
      alive = false;
      clearTimeout(timer);
      host?.removeEventListener('keydown', onkeydown);
      editor?.destroy();
    };
  });

  const blank = $derived(!text.trim());
  const defined = $derived(result?.ok ? result.defined : []);
  const canSave = $derived(edited && !busy && (blank || result?.ok === true));

  async function save() {
    if (!canSave) return;
    busy = true;
    error = null;
    try {
      const names = await putHooks(text);
      saved = text;
      done = true;
      store.notify(
        names.length ? `Hooks saved: ${names.join(', ')}.` : blank ? 'Hooks removed.' : 'Saved; no hook is defined.',
        'ok',
      );
    } catch (e) {
      error = e instanceof ApiError ? e.message : 'The hooks could not be saved.';
    } finally {
      busy = false;
    }
  }

  function onkeydown(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 's') {
      e.preventDefault();
      void save();
    }
  }
</script>

<section aria-label="Hooks">
  <h2>Hooks</h2>
  <p class="dim">
    A hook is a Lua function that runs at the same four moments Taskwarrior runs its scripts: <code>on_launch</code>
    before a command, <code>on_add</code> for each new task, <code>on_modify</code> for each task a command changes, and
    <code>on_exit</code> after it. They run on the server, so a script can hold secrets that never reach this page. Your
    desktop's hooks (<code>~/.task/hooks</code>) are separate. <code>rc.hooks:off</code> turns them off for a command.
  </p>

  {#if loadFailed}
    <div class="bad" role="alert"><TriangleAlert size={15} /> The hooks could not be read: {loadFailed}</div>
  {/if}
  {#if loading}<p class="dim" role="status">Loading…</p>{/if}

  <div class="editor" bind:this={host} hidden={loading || !!loadFailed}></div>

  {#if !loading && !loadFailed}
    <p class="status" role="status">
      {#if result && !result.ok}
        <span class="err"
          ><TriangleAlert size={14} />
          {result.line !== null ? `Line ${result.line}: ` : ''}{result.message}</span
        >
      {:else if blank}
        <span class="dim">No hooks. Start from an example, or write your own.</span>
      {:else if result?.ok}
        <span class="ok"
          >{defined.length ? `Defines ${defined.join(', ')}.` : 'Loads, but defines none of the four hooks.'}</span
        >
      {/if}
    </p>
    {#if error}<p class="err" role="alert">{error}</p>{/if}

    <p class="buttons">
      <button class="primary" type="button" onclick={save} disabled={!canSave}
        >{busy ? 'Saving…' : blank && edited ? 'Remove the hooks' : 'Save'}</button
      >
      <button type="button" onclick={() => editor?.setText(saved)} disabled={!edited || busy}>Discard changes</button>
      {#if blank}
        <button type="button" onclick={() => editor?.setText(HOOKS_EXAMPLE)}>Insert an example</button>
      {/if}
      {#if done && !edited}<span class="ok" role="status">Saved.</span>{/if}
    </p>

    <div class="secrets">
      <h3>Secrets</h3>
      <p class="dim">
        A secret is a value a script reads with <code>secret("name")</code> instead of having it in the script: a token
        for
        <code>http.post(…)</code>, say. It is kept sealed in your bucket with the same key as your tasks, can be
        replaced but never shown again, and is hidden from anything a hook prints or returns. (A Worker secret called
        <code>HOOK_SECRET_NAME</code>, set with <code>wrangler secret put</code> or in the dashboard, is read the same way
        and wins over one kept here.)
      </p>
      {#if secrets.length}
        <ul class="names" aria-label="Secrets that are set">
          {#each secrets as name (name)}
            <li>
              <code>{name}</code>
              <button type="button" class="ghost" aria-label="Remove {name}" onclick={() => removeSecret(name)}
                >Remove</button
              >
            </li>
          {/each}
        </ul>
      {/if}
      <form
        class="add-secret"
        onsubmit={(e) => {
          e.preventDefault();
          void keepSecret();
        }}
      >
        <input
          bind:value={secretName}
          placeholder="name (jira)"
          aria-label="Secret name"
          autocomplete="off"
          autocapitalize="off"
          spellcheck="false"
        />
        <input
          bind:value={secretValue}
          type="password"
          placeholder="value"
          aria-label="Secret value"
          autocomplete="off"
        />
        <button type="submit" disabled={!secretNameOk || !secretValue || secretBusy}
          >{secrets.includes(secretName) ? 'Replace' : 'Keep'}</button
        >
      </form>
      {#if secretError}<p class="err" role="alert">{secretError}</p>{/if}
    </div>

    <details class="help">
      <summary>What a hook gets</summary>
      <ul>
        <li>
          <strong>A task</strong> (<code>task</code>, <code>old</code>, <code>new</code>): <code>description</code>,
          <code>project</code>, <code>priority</code>, <code>status</code>, <code>uuid</code>,
          <code>blocked</code>, <code>blocking</code>, <code>recur</code>, <code>annotations</code> (a list of
          <code>&#123;entry, text&#125;</code>), and your UDAs as plain fields (<code>task.estimate = "big"</code>; set
          one to <code>nil</code> to remove it). <code>status</code>, <code>uuid</code>, annotations and recurrence can
          be read, not changed. Change the task you are given, or return one.
        </li>
        <li>
          <strong>Tags</strong> are a set: <code>task.tags:has("work")</code>, <code>:add("a", "b")</code>,
          <code>:remove("x")</code>, <code>:has_any(…)</code>, <code>:has_all(…)</code>, <code>:list()</code>,
          <code>#task.tags</code>, <code>for t in task.tags:each() do … end</code>. So is
          <code>task.depends</code> (task ids as text).
        </li>
        <li>
          <strong>Dates</strong> (<code>due</code>, <code>wait</code>, <code>scheduled</code>, <code>start</code>,
          <code>entry</code>, <code>modified</code>, <code>finished</code> for <code>end</code>, <code>expires</code>
          for
          <code>until</code>) are values with arithmetic: <code>task.due = tomorrow()</code>,
          <code>task.wait = task.due - days(2)</code>, <code>date("friday")</code>, <code>date("2026-12-25")</code>,
          <code>date("eom")</code>, <code>d1 &lt; d2</code>, <code>d2 - d1</code> (a duration),
          <code>d.year .month .day .hour .min .sec .weekday</code>, <code>d:format("Y-M-D")</code>,
          <code>d:start_of("week")</code>, <code>now()</code>, <code>today()</code>.
          <strong>Durations</strong>: <code>days(n)</code>, <code>hours(n)</code>, <code>minutes(n)</code>,
          <code>weeks(n)</code>, <code>duration("3d")</code>, <code>d:in_days()</code>.
        </li>
        <li>
          <strong>The network</strong>, to hosts the script declares at the top (<code
            >allow_hosts&#123;"acme.example.com"&#125;</code
          >), over https:
          <code
            >local res, err = http.post(url, &#123; headers = &#123; Authorization = "Bearer " .. secret("jira") &#125;,
            json = &#123; … &#125; &#125;)</code
          >
          (also <code>get</code>, <code>put</code>, <code>patch</code>, <code>delete</code>). <code>res</code> is
          <code>&#123;status, ok, headers, body, json&#125;</code>, or <code>nil</code> and a reason if the network failed,
          so a script can carry on. A few requests a call, 10 seconds at most each.
        </li>
        <li>
          <strong>Helpers</strong>: <code>task:in_project("Work")</code> (also true for <code>Work.Mail</code>),
          <code>:is_pending()</code>, <code>:is_waiting()</code>, <code>:is_overdue()</code>. <code>say(…)</code> and
          <code>warn(…)</code> print under the result; <code>refuse("why")</code> (or any error) stops the command. There
          are no files and no network, and a script that runs too long is stopped.
        </li>
      </ul>
    </details>
  {/if}
</section>

<style>
  section {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  h2 {
    margin: 0;
    font-size: 1.1rem;
  }
  .editor[hidden] {
    display: none;
  }
  .status {
    margin: 0;
    min-height: 1.4em;
    font-size: 13.5px;
  }
  .buttons {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin: 0;
  }
  .ok {
    color: var(--ok);
  }
  .err {
    color: var(--err);
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .bad {
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 8px 12px;
    border: 1px solid var(--err);
    border-radius: var(--radius);
    background: color-mix(in srgb, var(--err) 10%, var(--panel));
  }
  .secrets h3 {
    margin: 4px 0;
    font-size: 1rem;
  }
  .secrets .names {
    list-style: none;
    margin: 6px 0;
    padding: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .secrets .names li {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 2px 4px 2px 8px;
    border: 1px solid var(--line);
    border-radius: 999px;
  }
  .add-secret {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .help {
    font-size: 13.5px;
  }
  .help summary {
    cursor: pointer;
  }
  .help ul {
    margin: 8px 0 0;
    padding-left: 18px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
</style>
