import { describe, expect, it } from 'vitest';
import { exportFields, renderExport, tableFields } from './exportFile';
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
    expect(exportFields(TEXT)).toEqual(['id', 'description', 'tags', 'uuid']);
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
    expect(same.text).toBe(TEXT);
  });
});
