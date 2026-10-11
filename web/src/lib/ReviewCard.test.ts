// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import { review } from './review.svelte';
import ReviewCard from './ReviewCard.svelte';
import { store } from './store.svelte';
import type { ConfigResponse, Row } from './types';

const row = (description: string) =>
  ({
    uuid: 'u1',
    id: 1,
    description,
    status: 'pending',
    tags: [],
    annotations: [],
    extra: {},
    depends: [],
    urgency: 1,
    sessions: [],
    orphans: [],
    active_seconds: null,
  }) as unknown as Row;

let app: ReturnType<typeof mount> | null = null;
const tick = async () => {
  for (let i = 0; i < 4; i++) await Promise.resolve();
  flushSync();
};
const buttons = () => [...document.querySelectorAll<HTMLButtonElement>('dialog button')];
const button = (text: string) => buttons().find((b) => b.textContent?.trim() === text)!;
const mountCard = async () => {
  app = mount(ReviewCard, { target: document.body });
  await tick();
};

beforeEach(() => {
  HTMLDialogElement.prototype.showModal = function () {
    this.setAttribute('open', '');
  };
  store.config = {
    colors: {},
    config: { udas: {}, settings: {}, reports: {} },
    reports: [],
  } as unknown as ConfigResponse;
  review.surface = 'card';
  review.phase = 'running';
  review.row = row('Pay the rent');
  review.current = 1;
  review.total = 5;
  review.message = '';
  review.busy = false;
});
afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
  review.close();
  vi.restoreAllMocks();
});

describe('the review card', () => {
  it('shows where the review is, the task, and a button for each choice of the menu', async () => {
    await mountCard();
    expect(document.querySelector('[data-testid=review-count]')?.textContent).toBe('[2 of 5]');
    expect(document.querySelector('.title')?.textContent).toBe('Pay the rent');
    for (const name of ['Reviewed', 'Skip', 'Edit', 'Modify', 'Complete', 'Delete', 'Quit'])
      expect(button(name), name).toBeTruthy();
  });

  it('sends each choice to the session', async () => {
    const act = vi.spyOn(review, 'act').mockResolvedValue();
    await mountCard();
    for (const [name, key] of [
      ['Reviewed', 'reviewed'],
      ['Skip', 'skip'],
      ['Edit', 'edit'],
      ['Modify', 'modify'],
      ['Complete', 'complete'],
      ['Delete', 'delete'],
      ['Quit', 'quit'],
    ] as const) {
      button(name).click();
      expect(act).toHaveBeenLastCalledWith(key);
    }
  });

  it('has the focus on Reviewed, so Enter does what tasksh does', async () => {
    await mountCard();
    await tick();
    expect(document.activeElement).toBe(button('Reviewed'));
  });

  it('gets the focus onto Reviewed when it was opened while the review was still starting, and for each task', async () => {
    review.busy = true; // the card opens before the first task is there
    await mountCard();
    expect(document.activeElement).not.toBe(button('Reviewed'));
    review.busy = false;
    await tick();
    await tick();
    expect(document.activeElement).toBe(button('Reviewed'));
    // Another task: it is a new thing to answer, and the button is asked for the focus again.
    (document.activeElement as HTMLElement).blur();
    review.row = row('Water the plants');
    await tick();
    await tick();
    expect(document.activeElement).toBe(button('Reviewed'));
  });

  it('takes the letters of the menu, but not while something is typed', async () => {
    const act = vi.spyOn(review, 'act').mockResolvedValue();
    await mountCard();
    const dialog = document.querySelector('dialog')!;
    dialog.dispatchEvent(new KeyboardEvent('keydown', { key: 's', bubbles: true }));
    expect(act).toHaveBeenLastCalledWith('skip');
    dialog.dispatchEvent(new KeyboardEvent('keydown', { key: 'x', bubbles: true }));
    dialog.dispatchEvent(new KeyboardEvent('keydown', { key: 'd', ctrlKey: true, bubbles: true }));
    expect(act).toHaveBeenCalledTimes(1);
  });

  it('turns the choices off while one is being carried out', async () => {
    review.busy = true;
    await mountCard();
    expect(button('Reviewed').disabled).toBe(true);
    expect(button('Delete').disabled).toBe(true);
  });

  it('asks for the modification, and sends it', async () => {
    review.phase = 'modifying';
    const modify = vi.spyOn(review, 'modify').mockResolvedValue();
    await mountCard();
    const input = document.querySelector('input[aria-label=Modification]') as HTMLInputElement;
    input.value = '+tag project:X';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    document.querySelector('form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    expect(modify).toHaveBeenCalledWith('+tag project:X');
    button('Cancel').click();
    expect(modify).toHaveBeenLastCalledWith('');
  });

  it('asks the setup question with Yes and No', async () => {
    review.phase = 'setup';
    review.row = null;
    review.question = "Tasksh needs to define a 'reviewed' UDA of type 'date' for all tasks.  Ok to proceed?";
    const answer = vi.spyOn(review, 'answerSetup').mockResolvedValue();
    await mountCard();
    expect(document.querySelector('[data-testid=review-question]')?.textContent).toContain("'reviewed' UDA");
    button('Yes').click();
    expect(answer).toHaveBeenLastCalledWith(true);
    button('No').click();
    expect(answer).toHaveBeenLastCalledWith(false);
  });

  it('ends with the summary, until it is closed', async () => {
    review.phase = 'done';
    review.message = 'End of review. 3 out of 5 tasks reviewed.';
    await mountCard();
    expect(document.querySelector('.end')?.textContent).toBe('End of review. 3 out of 5 tasks reviewed.');
    button('Close').click();
    expect(review.phase).toBe('idle');
  });

  it('ends the review when the dialog itself is closed (Escape), leaving nothing half-open', async () => {
    await mountCard();
    document.querySelector('dialog')!.dispatchEvent(new Event('close'));
    expect(review.phase).toBe('idle');
  });

  it('shows the summary when it is quit with its own button', async () => {
    review.surface = 'card';
    await mountCard();
    (document.querySelector('button[aria-label="Quit the review"]') as HTMLButtonElement).click();
    await tick();
    expect(review.phase).toBe('done');
    expect(document.querySelector('.end')?.textContent).toContain('End of review.');
  });
});
