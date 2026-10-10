import { describe, expect, it } from 'vitest';
import {
  batches,
  describe as describeChanges,
  fieldsFor,
  filterWords,
  idList,
  modifyWords,
  namedWords,
  problem,
  promptFor,
} from './bulk';
import type { UdaDef } from './types';

const U1 = '11111111-1111-1111-1111-111111111111';
const U2 = '22222222-2222-2222-2222-222222222222';

describe('naming the selected tasks', () => {
  it('lists task numbers as short ranges', () => {
    expect(idList([1, 3, 4, 5, 9])).toBe('1,3-5,9');
    expect(idList([5, 3, 4])).toBe('3-5');
    expect(idList([1, 2])).toBe('1,2');
    expect(idList([7, 7, 7])).toBe('7');
    expect(idList([])).toBe('');
  });

  it('gives the numbers as one list and the whole uuid of a task that has none', () => {
    const tasks = [
      { uuid: U1, id: 3 },
      { uuid: U2, id: null },
      { uuid: 'x', id: 1 },
    ];
    expect(namedWords(tasks)).toEqual(['1,3', U2]);
    expect(promptFor(tasks)).toBe(`1,3 ${U2} `);
    expect(promptFor([{ uuid: U1, id: 4 }])).toBe('4 ');
  });

  it('names tasks to a button’s command by their whole uuids', () => {
    expect(
      filterWords([
        { uuid: U1, id: 3 },
        { uuid: U2, id: null },
      ]),
    ).toEqual([U1, U2]);
  });

  it('splits a long selection into commands the Worker will take', () => {
    const rows = Array.from({ length: 250 }, (_, i) => i);
    const parts = batches(rows);
    expect(parts.map((p) => p.length)).toEqual([120, 120, 10]);
    expect(parts.flat()).toEqual(rows);
    expect(batches([])).toEqual([]);
  });
});

const uda = (name: string, values: string[] = [], label: string | null = null): UdaDef => ({
  name,
  type: values.length ? 'string' : 'numeric',
  label,
  values,
  default: null,
  indicator: null,
});

describe('the Modify editor', () => {
  const fields = fieldsFor({ estimate: uda('estimate', [], 'Estimate'), size: uda('size', ['S', 'M']) });

  it('offers the usual fields, tags to add and remove, and the taskrc’s UDAs', () => {
    expect(fields.map((f) => f.key)).toEqual([
      'project',
      'priority',
      'due',
      'wait',
      'scheduled',
      'until',
      'recur',
      '+',
      '-',
      'depends',
      'estimate',
      'size',
    ]);
    expect(fields.find((f) => f.key === 'estimate')?.label).toBe('Estimate');
    expect(fields.find((f) => f.key === 'size')?.values).toEqual(['S', 'M']);
  });

  it('writes the words modify takes: field:value, +tag and -tag', () => {
    expect(
      modifyWords([
        { key: 'project', value: ' Home ' },
        { key: 'due', value: '' },
        { key: '+', value: 'a, +b  c' },
        { key: '-', value: 'old' },
        { key: 'estimate', value: '3' },
      ]),
    ).toEqual(['project:Home', 'due:', '+a', '+b', '+c', '-old', 'estimate:3']);
  });

  it('says what it will do', () => {
    expect(
      describeChanges(
        [
          { key: 'project', value: 'Home' },
          { key: 'due', value: '' },
          { key: '+', value: 'a b' },
          { key: '-', value: 'old' },
        ],
        fields,
      ),
    ).toBe('set project to Home, clear due, add +a +b, remove +old');
  });

  it('refuses an entry that cannot be applied: no field, or tags with none named', () => {
    expect(problem({ key: 'project', value: '' }, fields)).toBeNull(); // empty clears
    expect(problem({ key: '+', value: '  ' }, fields)).toMatch(/Add tags/);
    expect(problem({ key: 'nope', value: 'x' }, fields)).toMatch(/Choose a field/);
  });
});
