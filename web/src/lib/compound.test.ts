import { describe, expect, it } from 'vitest';
import { ChainError, runSequence, splitChain } from './compound';

const split = (line: string) => splitChain(line).map((s) => [s.join, s.line]);

describe('splitChain', () => {
  it('is one statement without an operator, as typed', () => {
    expect(split('  1 done  outcome:"a b" ')).toEqual([[null, '1 done  outcome:"a b"']]);
    expect(split('')).toEqual([]);
    expect(split(' ; ')).toEqual([]);
  });

  it('splits on &&, || and ;, with or without spaces', () => {
    expect(split('a && b || c ; d')).toEqual([
      [null, 'a'],
      ['&&', 'b'],
      ['||', 'c'],
      [';', 'd'],
    ]);
    expect(split('a&&b||c;d')).toEqual(split('a && b || c ; d'));
  });

  it('leaves a single & or | alone', () => {
    expect(split('add R&D | ops')).toEqual([[null, 'add R&D | ops']]);
  });

  it('keeps an operator in quotes, or after a backslash', () => {
    expect(split(`add 'a && b'`)).toEqual([[null, `add 'a && b'`]]);
    expect(split('add "x ; y" ; list')).toEqual([
      [null, 'add "x ; y"'],
      [';', 'list'],
    ]);
    expect(split('add a\\;b')).toEqual([[null, "add a';'b"]]);
    expect(split('+a \\|\\| +b list')).toEqual([[null, "+a '|''|' +b list"]]);
  });

  it('allows a trailing ;', () => {
    expect(split('a ;')).toEqual([[null, 'a']]);
    expect(split('a && b;')).toEqual([
      [null, 'a'],
      ['&&', 'b'],
    ]);
  });

  it('says what is missing', () => {
    expect(() => splitChain('&& a')).toThrow(new ChainError('Nothing before ‘&&’.'));
    expect(() => splitChain('a &&')).toThrow(new ChainError('Nothing after ‘&&’.'));
    expect(() => splitChain('a && ; b')).toThrow(new ChainError('Nothing between ‘&&’ and ‘;’.'));
    expect(() => splitChain('a ; ; b')).toThrow(ChainError);
  });
});

describe('runSequence', () => {
  /** Run `joins` where each statement succeeds or fails as `results` says; which ran, and the last status. */
  async function go(joins: ('&&' | '||' | ';' | null)[], results: boolean[]) {
    const ran: number[] = [];
    const ok = await runSequence(joins, async (i) => {
      ran.push(i);
      return results[i];
    });
    return { ran, ok };
  }

  it('runs the next after a success for &&, and not after a failure', async () => {
    expect(await go([null, '&&', '&&'], [true, true, true])).toEqual({ ran: [0, 1, 2], ok: true });
    expect(await go([null, '&&', '&&'], [true, false, true])).toEqual({ ran: [0, 1], ok: false });
  });

  it('runs the next after a failure for ||, and not after a success', async () => {
    expect(await go([null, '||'], [false, true])).toEqual({ ran: [0, 1], ok: true });
    expect(await go([null, '||'], [true, false])).toEqual({ ran: [0], ok: true });
  });

  it('runs the next whatever happened for ;', async () => {
    expect(await go([null, ';', ';'], [false, false, true])).toEqual({ ran: [0, 1, 2], ok: true });
  });

  it('goes left to right, and a skipped statement leaves the status as it was', async () => {
    // (a && b) || c: a fails, b is skipped, c runs.
    expect(await go([null, '&&', '||'], [false, true, true])).toEqual({ ran: [0, 2], ok: true });
    // (a || b) && c: a succeeds, b is skipped, c runs.
    expect(await go([null, '||', '&&'], [true, true, true])).toEqual({ ran: [0, 2], ok: true });
    // a || b || c: a succeeds, both are skipped.
    expect(await go([null, '||', '||'], [true, true, true])).toEqual({ ran: [0], ok: true });
  });
});
