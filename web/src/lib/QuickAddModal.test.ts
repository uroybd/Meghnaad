// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import QuickAddModal from './QuickAddModal.svelte';
import { store } from './store.svelte';

let app: ReturnType<typeof mount> | null = null;
let act: ReturnType<typeof vi.spyOn>;

beforeEach(() => {
  // jsdom has no modal dialogs.
  HTMLDialogElement.prototype.showModal = function () {
    this.setAttribute('open', '');
  };
  HTMLDialogElement.prototype.close = function () {
    this.removeAttribute('open');
    this.dispatchEvent(new Event('close'));
  };
  act = vi.spyOn(store, 'act').mockResolvedValue({ result: { kind: 'changed', tasks: [] } } as never);
  store.adding = null;
});
afterEach(() => {
  vi.restoreAllMocks();
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
});

const open = (props: object = {}) => {
  const onclose = vi.fn();
  app = mount(QuickAddModal, { target: document.body, props: { onclose, ...props } });
  flushSync();
  return onclose;
};
const field = () => document.querySelector('input[aria-label=Task]') as HTMLInputElement;
const type = (text: string) => {
  field().value = text;
  field().dispatchEvent(new Event('input', { bubbles: true }));
  flushSync();
};
const button = (name: RegExp) =>
  [...document.querySelectorAll('button')].find((b) => name.test(b.textContent ?? '')) as HTMLButtonElement;
const card = () => document.querySelector('[aria-label="Preview of the task"]') as HTMLElement;

describe('the one-line add', () => {
  it('has the line focused, an empty card, and nothing to add yet', () => {
    open();
    expect(document.activeElement).toBe(field());
    expect(card().textContent).toMatch(/will show here/);
    expect(button(/Add task/).disabled).toBe(true);
  });

  it('draws the task as the line is typed', () => {
    open();
    type('Buy milk project:Home +errand priority:H');
    expect(card().querySelector('.title')?.textContent).toBe('Buy milk');
    expect(card().textContent).toContain('Home');
    expect(card().querySelector('.tagpill')?.textContent).toBe('errand');
    expect(card().querySelector('.pri')?.textContent).toBe('H');
    expect(button(/Add task/).disabled).toBe(false);
  });

  it('adds with the words of the line and closes', async () => {
    const onclose = open();
    type('Buy milk due:2026-12-24 +home');
    document.querySelector('form')!.dispatchEvent(new Event('submit', { cancelable: true }));
    await vi.waitFor(() => expect(onclose).toHaveBeenCalled());
    expect(act).toHaveBeenCalledWith(null, ['add', 'Buy', 'milk', 'due:2026-12-24', '+home']);
  });

  it('says what is wrong and will not add it', () => {
    open();
    type('Pay rent due:nonsense');
    expect(document.querySelector('.problems')?.textContent).toMatch(/nonsense/);
    expect(button(/Add task/).disabled).toBe(true);
  });

  it('opens the full form with the line broken into its fields', () => {
    open();
    type('Buy milk project:Home +errand due:2026-12-24');
    button(/More fields/).click();
    expect(store.adding?.quick).toBe(false);
    expect(store.adding?.seed).toMatchObject({
      description: 'Buy milk',
      project: 'Home',
      tags: 'errand',
      due: '2026-12-24',
    });
  });
});
