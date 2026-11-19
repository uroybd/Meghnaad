// @vitest-environment jsdom
import { afterEach, expect, it } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import { lazy } from './lazy.svelte';
import LazyView from './LazyView.svelte';
import Shown from './WaitBar.svelte';
import { waiting } from './waiting.svelte';

let app: ReturnType<typeof mount> | null = null;
const tick = async () => {
  for (let i = 0; i < 5; i++) await new Promise((r) => setTimeout(r, 0));
  flushSync();
};

afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
});

it('draws the component once its code is here, counting the wait meanwhile', async () => {
  let arrive: (m: { default: typeof Shown }) => void = () => {};
  const part = lazy(() => new Promise((r) => (arrive = r)));
  app = mount(LazyView, { target: document.body, props: { of: part } });
  flushSync();
  expect(waiting.count).toBe(1);
  arrive({ default: Shown });
  await tick();
  expect(waiting.count).toBe(0);
  expect(part.component).toBe(Shown);
  expect(document.querySelector('[role=alert]')).toBeNull();
});

it('says so when the code cannot be fetched (offline), offers Reload, and Try again fetches it again', async () => {
  let tries = 0;
  const part = lazy(() => {
    tries++;
    return tries === 1 ? Promise.reject(new Error('offline')) : Promise.resolve({ default: Shown });
  });
  app = mount(LazyView, { target: document.body, props: { of: part } });
  await tick();
  const alert = document.querySelector('[role=alert]');
  expect(alert?.textContent).toContain('could not be loaded');
  const buttons = [...(alert?.querySelectorAll('button') ?? [])].map((b) => b.textContent?.trim());
  expect(buttons).toEqual(['Reload', 'Try again']);
  (alert?.querySelectorAll('button')[1] as HTMLButtonElement).click();
  await tick();
  expect(tries).toBe(2);
  expect(part.component).toBe(Shown);
  expect(document.querySelector('[role=alert]')).toBeNull();
});
