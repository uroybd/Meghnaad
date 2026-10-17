// @vitest-environment jsdom
import { describe, expect, it } from 'vitest';
import { addArgs, previewAdd, seedOf } from './addPreview';
import type { UdaDef } from './types';

describe('breaking a line down as `add` reads it', () => {
  it('finds the description and the fields', () => {
    const p = previewAdd('Buy milk project:Home priority:H +errand due:2026-12-24', null);
    expect(p).toMatchObject({ description: 'Buy milk', project: 'Home', priority: 'H', tags: ['errand'], errors: [] });
    expect(Object.keys(p.dates)).toEqual(['due']);
  });

  it('keeps a quoted word in the description and sends the words the same way', () => {
    expect(previewAdd("'read project:x' later", null)).toMatchObject({
      description: 'read project:x later',
      project: null,
    });
    expect(addArgs("'read project:x' later +a")).toEqual(['add', 'read project:x', 'later', '+a']);
  });

  it('reports a part that is wrong and still shows the rest', () => {
    const p = previewAdd('Pay rent due:nonsense +bills', null);
    expect(p.description).toBe('Pay rent');
    expect(p.tags).toEqual(['bills']);
    expect(p.errors).toHaveLength(1);
  });

  it('knows the UDAs of the config', () => {
    const udas = {
      estimate: { name: 'estimate', type: 'numeric', label: null, values: [], default: null, indicator: null },
    };
    const p = previewAdd('Fix estimate:3', { udas } as never);
    expect(p.udas).toEqual({ estimate: '3' });
    expect(p.description).toBe('Fix');
  });
});

describe('the fields of the form for a line', () => {
  it('fills each from the breakdown', () => {
    const seed = seedOf(previewAdd('Ship it project:Work +a +b priority:M due:2026-12-24T09:30 depends:3', null));
    expect(seed).toMatchObject({
      description: 'Ship it',
      project: 'Work',
      priority: 'M',
      tags: 'a b',
      due: '2026-12-24T09:30',
      depends: '3',
    });
  });

  it('writes a date UDA as the form holds a date, and carries what has no field', () => {
    const defs: Record<string, UdaDef> = {
      seen: { name: 'seen', type: 'date', label: null, values: [], default: null, indicator: null },
    };
    const p = previewAdd('Look seen:2026-12-01 start:2026-12-02', { udas: defs } as never);
    const seed = seedOf(p, '', defs);
    expect(seed.udas.seen).toBe('2026-12-01');
    expect(seed.carried).toEqual(['start:2026-12-02']);
  });

  it('is empty but for the text when there is no breakdown', () => {
    expect(seedOf(null, 'hello')).toMatchObject({ description: 'hello', project: '', tags: '', due: '' });
  });
});
