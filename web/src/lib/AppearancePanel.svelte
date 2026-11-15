<script lang="ts">
  import { untrack } from 'svelte';
  import {
    DEFAULT_PROGRESS,
    progressProblem,
    sameAppearance,
    type Appearance,
    type Progress,
    type Transformation,
  } from './appsettings';
  import { ArrowDown, ArrowUp, Check, Plus, X } from './icons';
  import { transformProblems } from './markdown';
  import { store } from './store.svelte';

  /** There are edits not yet applied (the tab shows a dot). */
  let { ondirty }: { ondirty?: (dirty: boolean) => void } = $props();

  const saved = $derived(store.appearance);
  // What is typed: for each UDA shown as a bar, its min and max as text (so a half-typed number is not lost).
  type Draft = Record<string, { min: string; max: string }>;
  const toDraft = (a: Appearance): Draft =>
    Object.fromEntries(Object.entries(a.progress).map(([n, p]) => [n, { min: String(p.min), max: String(p.max) }]));
  let draft = $state<Draft>(untrack(() => toDraft(saved)));
  // The transformations as typed, each with a key of its own for the list.
  type RuleDraft = Transformation & { key: number };
  let nextKey = 0;
  const toRules = (a: Appearance): RuleDraft[] =>
    a.transformations.map((r) => ({ ...r, fields: [...r.fields], key: nextKey++ }));
  let rules = $state<RuleDraft[]>(untrack(() => toRules(saved)));
  let busy = $state(false);
  let error = $state<string | null>(null);
  let done = $state(false);

  /** The numeric UDAs the taskrc defines, and any that are saved as bars but not defined (any more), so they can be turned off. */
  const names = $derived([
    ...new Set([
      ...Object.values(store.config?.config.udas ?? {})
        .filter((u) => u.type === 'numeric')
        .map((u) => u.name),
      ...Object.keys(draft),
    ]),
  ]);

  const range = (name: string): Progress => ({ min: Number(draft[name].min), max: Number(draft[name].max) });
  const problem = (name: string) => {
    const d = draft[name];
    if (!d) return null;
    if (d.min.trim() === '' || d.max.trim() === '') return 'min and max are needed';
    return progressProblem(range(name));
  };
  /** The string UDAs the taskrc defines: the fields a transformation may name besides the description and the notes. */
  const stringUdas = $derived(
    Object.values(store.config?.config.udas ?? {})
      .filter((u) => u.type === 'string')
      .map((u) => u.name),
  );
  /** The fields to offer a rule: the text fields, and any it names that are not (any more), so they can be taken off. */
  const fieldsOf = (r: RuleDraft) => [...new Set(['description', 'annotations', ...stringUdas, ...r.fields])];
  const current = $derived<Appearance>({
    progress: Object.fromEntries(Object.keys(draft).map((n) => [n, range(n)])),
    transformations: rules.map(({ match, replace, fields }) => ({ match, replace, fields })),
  });
  // What the engine says is wrong with the rules (a pattern that does not compile, no fields, a field that is not text).
  const ruleProblems = $derived(transformProblems(current.transformations, stringUdas));
  const invalid = $derived(Object.keys(draft).some((n) => problem(n)) || ruleProblems.length > 0);
  const edited = $derived(!sameAppearance(current, saved));
  $effect(() => {
    ondirty?.(edited);
  });

  // Saved somewhere else (another tab, the file): follow it, unless there are edits here.
  let before = untrack(() => saved);
  $effect(() => {
    const now = saved;
    const untouched = sameAppearance(
      untrack(() => current),
      before,
    );
    const changed = !sameAppearance(now, before);
    before = now;
    if (changed && untouched) {
      draft = toDraft(now);
      rules = toRules(now);
    }
  });

  function addRule() {
    done = false;
    rules.push({ match: '', replace: '', fields: ['description'], key: nextKey++ });
  }

  function moveRule(i: number, by: number) {
    done = false;
    const [r] = rules.splice(i, 1);
    rules.splice(i + by, 0, r);
  }

  function pickField(r: RuleDraft, field: string, on: boolean) {
    done = false;
    r.fields = on ? [...r.fields, field] : r.fields.filter((f) => f !== field);
  }

  function toggle(name: string, on: boolean) {
    done = false;
    if (on) draft[name] = { min: String(DEFAULT_PROGRESS.min), max: String(DEFAULT_PROGRESS.max) };
    else delete draft[name];
  }

  async function apply() {
    busy = true;
    error = null;
    done = false;
    try {
      await store.saveAppearance(current);
      done = true;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }
</script>

<section aria-label="Appearance settings">
  <h2>Progress bars</h2>
  <p class="dim">
    A numeric UDA shown as a bar in reports: empty at its minimum, full at its maximum. A value outside the range fills
    or empties the bar; a task with no value shows nothing. It is kept in this app's own
    <code>settings.json</code>, not in your taskrc.
  </p>
  {#if store.appSettings?.appearanceErrors.length}
    <p class="err" role="alert">{store.appSettings.appearanceErrors.join(' ')}</p>
  {/if}
  {#if names.length === 0}
    <p class="dim">No numeric UDAs yet. Define one in the taskrc (<code>uda.progress.type=numeric</code>).</p>
  {/if}
  <ul class="udas">
    {#each names as name (name)}
      {@const on = name in draft}
      <li>
        <label class="pick">
          <input type="checkbox" checked={on} onchange={(e) => toggle(name, e.currentTarget.checked)} />
          <span class="name">{name}</span>
        </label>
        {#if on}
          <label class="num">
            <span>min</span>
            <input type="number" step="any" bind:value={draft[name].min} oninput={() => (done = false)} />
          </label>
          <label class="num">
            <span>max</span>
            <input type="number" step="any" bind:value={draft[name].max} oninput={() => (done = false)} />
          </label>
          {#if problem(name)}<span class="err" role="alert">{problem(name)}</span>{/if}
        {/if}
      </li>
    {/each}
  </ul>

  <h2 class="next">Transformations</h2>
  <p class="dim">
    Find and replace in a text field before it is drawn, so <code>BOR-23434</code> can become a link: the result is
    shown as Markdown. <em>Match</em> is a regular expression, <em>Replace</em> may use <code>$1</code> or
    <code>{'${name}'}</code>, and each runs only on the fields ticked. They run in order, on what the one before left.
    Only how a field is shown changes: the task, filters and exports keep what you typed.
  </p>
  {#if rules.length}
    <ol class="rules" aria-label="Transformations">
      {#each rules as r, i (r.key)}
        <li>
          <div class="rule-row">
            <input
              class="mono grow"
              bind:value={r.match}
              oninput={() => (done = false)}
              aria-label="Match"
              placeholder="\b(BOR-\d+)\b"
              spellcheck="false"
              autocomplete="off"
              autocapitalize="off"
            />
            <input
              class="mono grow"
              bind:value={r.replace}
              oninput={() => (done = false)}
              aria-label="Replace"
              placeholder="[$1](https://jira.example.com/browse/$1)"
              spellcheck="false"
              autocomplete="off"
              autocapitalize="off"
            />
            <button
              type="button"
              class="ghost"
              aria-label="Move up"
              title="Move up"
              disabled={i === 0}
              onclick={() => moveRule(i, -1)}><ArrowUp size={15} /></button
            >
            <button
              type="button"
              class="ghost"
              aria-label="Move down"
              title="Move down"
              disabled={i === rules.length - 1}
              onclick={() => moveRule(i, 1)}><ArrowDown size={15} /></button
            >
            <button
              type="button"
              class="ghost"
              aria-label="Remove transformation {i + 1}"
              title="Remove"
              onclick={() => ((rules = rules.filter((x) => x.key !== r.key)), (done = false))}><X size={15} /></button
            >
          </div>
          <div class="fields" role="group" aria-label="Fields for transformation {i + 1}">
            {#each fieldsOf(r) as f (f)}
              <label class="pick">
                <input
                  type="checkbox"
                  checked={r.fields.includes(f)}
                  onchange={(e) => pickField(r, f, e.currentTarget.checked)}
                />
                <span class="name">{f}</span>
              </label>
            {/each}
          </div>
        </li>
      {/each}
    </ol>
  {:else}
    <p class="dim">No transformations yet.</p>
  {/if}
  {#if ruleProblems.length}
    <ul class="err" role="alert">
      {#each ruleProblems as p, i (i)}<li>{p}</li>{/each}
    </ul>
  {/if}
  <button type="button" class="btn" onclick={addRule}><Plus size={14} /> Add a transformation</button>

  {#if error}<p class="err" role="alert">{error}</p>{/if}
  <div class="row">
    <button class="primary" onclick={apply} disabled={busy || !edited || invalid}>Apply</button>
    {#if done}<span class="ok"><Check size={14} /> Saved</span>{/if}
  </div>
</section>

<style>
  h2 {
    font-size: 16px;
    margin: 0 0 10px;
  }
  h2.next {
    margin-top: 24px;
  }
  .rules {
    margin: 0 0 12px;
    padding-left: 20px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .rule-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .rule-row .grow {
    flex: 1 1 200px;
    min-width: 0;
  }
  .fields {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 16px;
    margin-top: 6px;
  }
  .fields .pick {
    min-width: 0;
  }
  ul.err {
    margin: 0 0 12px;
    padding-left: 20px;
  }
  .udas {
    list-style: none;
    margin: 0 0 12px;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 12px;
  }
  .pick {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    min-width: 140px;
  }
  .name {
    font-family: var(--mono, monospace);
  }
  .num {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--dim);
  }
  .num input {
    width: 90px;
    font-size: 14px;
    color: var(--text);
  }
</style>
