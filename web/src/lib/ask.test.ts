import { describe, expect, it } from 'vitest';
import { ask, asker } from './ask.svelte';

describe('the page’s own question', () => {
  it('answers each question in turn, the next waiting until the first is answered', async () => {
    const first = ask('First?');
    const second = ask('Second?', { yes: 'Delete' });
    expect(asker.current?.message).toBe('First?');
    asker.answer(true);
    expect(await first).toBe(true);
    expect(asker.current).toMatchObject({ message: 'Second?', yes: 'Delete', no: 'No' });
    asker.answer(false);
    expect(await second).toBe(false);
    expect(asker.current).toBeNull();
  });
});
