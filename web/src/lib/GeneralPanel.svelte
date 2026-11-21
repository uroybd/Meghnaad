<script lang="ts">
  import { ask } from './ask.svelte';
  import { untrack } from 'svelte';
  import { STATIC } from '../engine/flavour';
  import DevicePanel from './DevicePanel.svelte';
  import { putAppSettings } from './api';
  import { lintSettings, parseHomepage, parseSettings, sameHomepage, type Homepage } from './appsettings';
  import { ensureDeviceId } from './deviceAsk.svelte';
  import { deviceIdProblem, withDeviceHomepage } from './deviceSettings';
  import { Check, Copy, TriangleAlert } from './icons';
  import { resolveKeymap } from './keymap';
  import { store } from './store.svelte';
  import { strayAliasIcons } from './taskCommands';

  /** There are edits not yet applied (the tab shows a dot). */
  let { ondirty }: { ondirty?: (dirty: boolean) => void } = $props();

  // The choice is one value in the list: `report:next`, `board:week`, `kanban:work`, or nothing for the default (`next`).
  const toValue = (h: Homepage | null) =>
    !h ? '' : 'report' in h ? `report:${h.report}` : 'board' in h ? `board:${h.board}` : `kanban:${h.kanban}`;
  const fromValue = (v: string): Homepage | null =>
    v.startsWith('report:')
      ? { report: v.slice(7) }
      : v.startsWith('board:')
        ? { board: v.slice(6) }
        : v.startsWith('kanban:')
          ? { kanban: v.slice(7) }
          : null;
  // The kanbans are offered while Kanban is on.
  const kanbans = $derived(store.kanbanSettings.enabled ? store.kanbans : []);

  // Under This device (spec 0027): the home page this device uses, and whether it is its own.
  const device = $derived(store.settingsDevice);
  const own = $derived(!!store.deviceRaw && 'homepage' in store.deviceRaw);
  const everyones = $derived(store.appSettings?.homepage ?? null);
  const saved = $derived(device ? store.homepage : everyones);
  let choice = $state(untrack(() => toValue(saved)));
  let busy = $state(false);
  let error = $state<string | null>(null);
  let done = $state(false);

  const edited = $derived(!sameHomepage(fromValue(choice), saved));

  // Saved somewhere else (another tab, the file): follow it, unless there are edits here.
  let before = untrack(() => saved);
  $effect(() => {
    const now = saved;
    const untouched = sameHomepage(fromValue(choice), before);
    const changed = !sameHomepage(now, before);
    before = now;
    if (changed && untouched) choice = toValue(now);
  });

  // The saved choice may name a report, board or kanban that is not there (any more): say so rather than show a blank list.
  const missing = $derived(
    !saved
      ? false
      : 'report' in saved
        ? !store.reports.some((r) => r.name === saved.report)
        : 'board' in saved
          ? !store.boards.some((b) => b.id === saved.board)
          : !kanbans.some((k) => k.id === saved.kanban),
  );

  // The whole of settings.json as it is saved (or this device's file, under This device), to read, edit or paste over,
  // and to copy to the terminal app or the phone.
  const file = $derived(JSON.stringify((device ? store.deviceRaw : store.appSettings?.raw) ?? {}, null, 2) + '\n');
  const fileName = $derived(device ? `devices/${store.deviceId ?? ''}.json` : 'settings.json');
  let text = $state(untrack(() => file));
  const textEdited = $derived(text !== file);
  // Saved somewhere else: follow it, unless the text has edits.
  let fileBefore = untrack(() => file);
  $effect(() => {
    const now = file;
    const untouched = untrack(() => text) === fileBefore;
    fileBefore = now;
    if (untouched) text = now;
  });
  const lint = $derived(
    lintSettings(text, (raw) => [
      ...resolveKeymap(raw, store.keymapDefaults).problems,
      ...strayAliasIcons(parseSettings(JSON.stringify(raw)).aliasIcons, store.config?.config.settings ?? {}),
    ]),
  );
  let fileBusy = $state(false);
  let fileError = $state<string | null>(null);
  let fileDone = $state(false);

  $effect(() => {
    ondirty?.(edited || textEdited);
  });

  async function applyFile() {
    fileBusy = true;
    fileError = null;
    fileDone = false;
    try {
      // Saved as written: what is pasted is the file.
      if (device) await store.saveDeviceText(text);
      else {
        await putAppSettings(text);
        await store.loadAppSettings();
      }
      text = file;
      fileDone = true;
    } catch (e) {
      fileError = e instanceof Error ? e.message : String(e);
    } finally {
      fileBusy = false;
    }
  }

  /** Under This device: the home page everyone's again. */
  async function putBack() {
    busy = true;
    error = null;
    try {
      await store.saveDevice(withDeviceHomepage(store.deviceRaw ?? {}, undefined));
      choice = toValue(everyones);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  // This device's ID, to change (which renames its file, or, for an ID another device has, uses that one's).
  // Typed over, and back to the saved ID whenever that changes.
  let idText = $derived(store.deviceId ?? '');
  const idProblem = $derived(deviceIdProblem(idText.trim()));
  let idBusy = $state(false);
  let idError = $state<string | null>(null);
  async function rename() {
    const next = idText.trim();
    idBusy = true;
    idError = null;
    try {
      const taken = (await store.listDevices()).includes(next);
      if (taken && !(await ask(`The device ${next} has settings already. Use those here, in place of this device's?`)))
        return;
      if (taken) await store.useDevice(next);
      else await store.saveDevice(store.deviceRaw ?? {}, next);
    } catch (e) {
      idError = e instanceof Error ? e.message : String(e);
    } finally {
      idBusy = false;
    }
  }
  async function forget() {
    const id = store.deviceId;
    if (
      !id ||
      !(await ask(`Delete this device's own settings (${id})? It uses everyone's again, as does any device on ${id}.`))
    )
      return;
    try {
      await store.deleteDevice(id);
    } catch (e) {
      idError = e instanceof Error ? e.message : String(e);
    }
  }

  let copied = $state(false);
  async function copy() {
    try {
      await navigator.clipboard.writeText(text);
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch {
      store.notify('Could not copy: select the text and copy it by hand.', 'err');
    }
  }

  async function apply() {
    busy = true;
    error = null;
    done = false;
    try {
      if (device) {
        const id = await ensureDeviceId();
        if (!id) return;
        await store.saveDevice(withDeviceHomepage(store.deviceRaw ?? {}, fromValue(choice)), id);
      } else await store.saveHomepage(fromValue(choice));
      done = true;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }
</script>

<section aria-label="General settings">
  <h2>General</h2>
  {#if device && store.deviceId}
    <div class="device">
      <label class="field">
        <span>Device ID</span>
        <input bind:value={idText} spellcheck="false" autocomplete="off" autocapitalize="off" aria-label="Device ID" />
      </label>
      <div class="row">
        <button onclick={rename} disabled={idBusy || !!idProblem || idText.trim() === store.deviceId}>Change</button>
        <button class="ghost" onclick={forget} disabled={idBusy}>Delete this device's settings</button>
      </div>
      {#if idProblem && idText.trim()}<p class="err" role="alert">{idProblem}</p>{/if}
      {#if idError}<p class="err" role="alert">{idError}</p>{/if}
    </div>
  {:else if device}
    <p class="dim">
      This device has no settings of its own yet: it uses everyone's. Saving something here makes them, and asks for a
      name.
    </p>
  {/if}
  <div class="own" class:mine={device && own}>
    <label class="field">
      <span
        >Home page{#if device && own}<span class="tag">this device's</span>{/if}</span
      >
      <select bind:value={choice} onchange={() => (done = false)}>
        <option value="">next (the default)</option>
        <optgroup label="Reports">
          {#each store.offeredReports as r (r.name)}
            {#if r.name !== 'next'}<option value="report:{r.name}">{r.name}</option>{/if}
          {/each}
        </optgroup>
        {#if store.boards.length}
          <optgroup label="Boards">
            {#each store.boards as b (b.id)}<option value="board:{b.id}">{b.name}</option>{/each}
          </optgroup>
        {/if}
        {#if kanbans.length}
          <optgroup label="Kanbans">
            {#each kanbans as k (k.id)}<option value="kanban:{k.id}">{k.name}</option>{/each}
          </optgroup>
        {/if}
        {#if choice && !store.offeredReports.some((r) => choice === `report:${r.name}`) && !store.boards.some((b) => choice === `board:${b.id}`) && !kanbans.some((k) => choice === `kanban:${k.id}`)}
          <option value={choice}>{choice.slice(choice.indexOf(':') + 1)} (not found)</option>
        {/if}
      </select>
    </label>
    {#if device && own}
      <button class="small ghost" onclick={putBack} disabled={busy}
        >Use everyone's ({toValue(everyones) || 'next'})</button
      >
    {/if}
  </div>
  {#if !device && store.deviceRaw && 'homepage' in store.deviceRaw}
    <p class="dim">
      This device uses its own:
      <code>{toValue(parseHomepage(store.deviceRaw.homepage)) || 'next'}</code>
      (<a href="/settings/device/general" onclick={(e) => (e.preventDefault(), (store.settingsDevice = true))}
        >under This device</a
      >).
    </p>
  {/if}
  <p class="dim">
    The page the app opens on: any report, including your own, any of your boards, or a kanban while Kanban is on. Left
    alone it is
    <code>next</code>. A link to a task still opens that task. It is kept in this app's own <code>settings.json</code>,
    not in your taskrc.
  </p>
  {#if missing}
    <p class="err" role="alert">
      The saved home page is not there any more, so the app opens on <code>next</code>.
    </p>
  {/if}
  {#if error}<p class="err" role="alert">{error}</p>{/if}
  <div class="row">
    <button class="primary" onclick={apply} disabled={busy || !edited}>Apply</button>
    {#if done}<span class="ok"><Check size={14} /> Saved</span>{/if}
  </div>

  {#if !device || store.deviceId}
    <details class="json" open={textEdited || undefined}>
      <summary>{fileName}</summary>
      {#if device}
        <p class="dim">
          This device's own settings, laid over everyone's: its <code>deviceId</code>, and its home page, keys and
          reminders where it has them. Edit it here or paste a whole file over it.
        </p>
      {:else}
        <p class="dim">
          This app's own settings: the home page, appearance, task commands, boards and keys, and the terminal app's and
          the phone's own. Edit it here or paste a whole file over it; what this app doesn't use is kept as it is. The
          terminal app reads the same file from <code>~/.config/meghnaad</code>.
        </p>
      {/if}
      <div class="file">
        <button type="button" class="ghost copy" onclick={copy} aria-label="Copy {fileName}">
          {#if copied}<Check size={13} /> Copied{:else}<Copy size={13} /> Copy{/if}
        </button>
        <textarea
          class="mono"
          bind:value={text}
          oninput={() => (fileDone = false)}
          aria-label={fileName}
          spellcheck="false"
          autocomplete="off"
          autocapitalize="off"
          rows="18"></textarea>
      </div>
      {#if lint.errors.length || lint.warnings.length}
        <ul class="lint" aria-label="Problems in {fileName}">
          {#each lint.errors as e, i (i)}<li class="err"><TriangleAlert size={13} /> {e}</li>{/each}
          {#each lint.warnings as w, i (i)}<li class="warn"><TriangleAlert size={13} /> {w} (kept)</li>{/each}
        </ul>
      {/if}
      {#if fileError}<p class="err" role="alert">{fileError}</p>{/if}
      <div class="row">
        <button
          class="primary"
          onclick={applyFile}
          disabled={fileBusy || !textEdited || lint.errors.length > 0}
          title={lint.errors.length ? 'Put the errors right first' : undefined}>Apply</button
        >
        <button type="button" class="ghost" onclick={() => (text = file)} disabled={fileBusy || !textEdited}
          >Reset</button
        >
        {#if fileDone}<span class="ok"><Check size={14} /> Saved</span>{/if}
      </div>
    </details>
  {/if}
  {#if STATIC}<DevicePanel />{/if}
</section>

<style>
  .field {
    display: flex;
    flex-direction: column;
    gap: 3px;
    max-width: 360px;
    margin-bottom: 8px;
    font-size: 12px;
    color: var(--dim);
  }
  .field select {
    font-size: 14px;
    color: var(--text);
  }
  .json {
    margin-top: 20px;
  }
  .own {
    border: 1px solid transparent;
    border-radius: var(--radius);
    max-width: 380px;
  }
  /* A field this device has its own value for (spec 0027): its name and value framed together. */
  .own.mine {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 7%, transparent);
    padding: 8px 10px;
    margin: 0 0 8px -11px;
  }
  .tag {
    margin-left: 6px;
    font-size: 11px;
    color: var(--accent);
  }
  .device {
    margin-bottom: 16px;
  }
  .device input {
    font-family: var(--mono);
    font-size: 14px;
    color: var(--text);
  }
  .json summary {
    cursor: pointer;
    font-family: var(--mono);
    font-size: 13px;
  }
  .file {
    position: relative;
  }
  .file textarea {
    display: block;
    box-sizing: border-box;
    width: 100%;
    margin: 6px 0 0;
    padding: 10px 12px;
    background: var(--panel-2);
    border-radius: var(--radius);
    font-size: 12.5px;
    resize: vertical;
    max-height: 70vh;
  }
  .lint {
    list-style: none;
    margin: 8px 0 0;
    padding: 0;
    font-size: 12.5px;
  }
  .lint li {
    display: flex;
    gap: 6px;
    align-items: baseline;
  }
  .lint .warn {
    color: var(--dim);
  }
  .copy {
    position: absolute;
    top: 6px;
    right: 6px;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    background: var(--panel);
  }
  h2 {
    font-size: 16px;
    margin: 0 0 10px;
  }
</style>
