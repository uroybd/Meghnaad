import { describe, expect, it } from 'vitest';
import { exportFields, renderExport, tableFields, type ExportChoice } from './exportFile';
import { contextWords, exportArgs } from './exportScope';
import type { ConfigResponse } from './types';

// The engine's `table_export` does the work and has the tests of what it means; these show the page reaches it.
const TEXT = '[\n{"id":1,"description":"Buy, milk","tags":["a","b"],"uuid":"u1"}\n]\n';
const cfg = {
  udas: {},
  reports: {},
  contexts: { home: { name: 'home', read: 'project:Home', write: null, rc: {} } },
  active_context: 'home',
  urgency: {},
  settings: {},
} as unknown as ConfigResponse['config'];

describe('the Export button, through the engine', () => {
  it('knows what a table exports, and the context in front of a filter’s export', () => {
    expect(exportArgs({ line: '+x next' }, undefined, cfg)).toEqual(['(', 'project:Home', ')', '+x', 'export', 'next']);
    expect(exportArgs({ args: ['next'] }, 'none', cfg)).toEqual(['export', 'next']);
    expect(exportArgs({ line: 'calendar' }, undefined, cfg)).toBeNull();
    expect(contextWords(['+x'], undefined, cfg)).toEqual(['(', 'project:Home', ')']);
  });

  it('shapes the file', () => {
    expect(exportFields(TEXT)).toEqual(['id', 'description', 'tags', 'uuid', 'time_tracking']);
    expect(tableFields(['id', 'due.relative'])).toEqual(['id', 'due']);
    const csv = renderExport(
      { name: 'tasks-next-20261116-010203.json', text: TEXT },
      { format: 'csv', array: null, fields: 'custom', picked: ['description', 'tags'] },
      [],
      cfg,
    );
    expect(csv).toEqual({
      name: 'tasks-next-20261116-010203.csv',
      text: 'description,tags\r\n"Buy, milk",a b\r\n',
      mime: 'text/csv',
    });
    const same = renderExport(
      { name: 'x.json', text: TEXT },
      { format: 'json', array: null, fields: 'all', picked: [] },
      [],
      cfg,
    );
    // Every field: what `export` wrote, each task with its tracked time after its own fields.
    expect(same.text).toBe(TEXT.replace('"u1"}', '"u1","time_tracking":{"total":0,"sessions":[]}}'));
  });

  it('writes TSV, a task to a line, and dates a spreadsheet reads, in UTC', () => {
    const text = '[\n{"description":"two\\nlines","due":"20261119T180000Z","uuid":"u1"}\n]\n';
    const choice: ExportChoice = { format: 'tsv', array: null, fields: 'all', picked: [] };
    expect(renderExport({ name: 'tasks.json', text }, choice, [], cfg)).toEqual({
      name: 'tasks.tsv',
      text: 'description\tdue\tuuid\ttime_tracking\ntwo\\nlines\t2026-11-19 18:00:00\tu1\t0\n',
      mime: 'text/tab-separated-values',
    });
    const csv = renderExport({ name: 'tasks.json', text }, { ...choice, format: 'csv' }, [], cfg);
    expect(csv.text).toBe('description,due,uuid,time_tracking\r\n"two\nlines",2026-11-19 18:00:00,u1,0\r\n');
  });
});
