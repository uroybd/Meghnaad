import { describe, expect, it } from 'vitest';
import {
  configArgs,
  menuKey,
  parseReview,
  partLines,
  REVIEW_FILTER,
  REVIEW_SORT,
  setupLines,
  setupQuestion,
  taskshStatus,
} from './tasksh';
import type { ConfigResponse } from './types';

const WANT = { filter: REVIEW_FILTER, sort: REVIEW_SORT };

/** Just the parts of the response these functions read. */
function config(opts: { uda?: { type: string; label: string | null }; report?: Record<string, unknown> } = {}) {
  const col = (spec: string) => ({ spec, name: spec, format: null, label: spec, kind: 'string' });
  return {
    config: { udas: opts.uda ? { reviewed: { name: 'reviewed', ...opts.uda } } : {} },
    reports: opts.report
      ? [
          {
            name: '_reviewed',
            description: 'x',
            columns: ((opts.report.columns as string[]) ?? ['uuid']).map(col),
            filter: (opts.report.filter as string) ?? REVIEW_FILTER,
            sort: (opts.report.sort as string) ?? REVIEW_SORT,
          },
        ]
      : [{ name: 'next', description: null, columns: [], filter: null, sort: null }],
  } as unknown as ConfigResponse;
}

describe('taskshStatus', () => {
  it('is not enabled until the UDA and the report are both there', () => {
    expect(taskshStatus(null)).toMatchObject({ uda: 'missing', report: 'missing', enabled: false });
    const uda = { type: 'date', label: 'Reviewed' };
    expect(taskshStatus(config({ uda }))).toMatchObject({ uda: 'ok', report: 'missing', enabled: false });
    expect(taskshStatus(config({ report: {} }))).toMatchObject({ uda: 'missing', report: 'ok', enabled: false });
    expect(taskshStatus(config({ uda, report: {} }))).toMatchObject({ uda: 'ok', report: 'ok', enabled: true });
  });

  it('says what is there when it is not the way tasksh needs it', () => {
    const s = taskshStatus(config({ uda: { type: 'string', label: null }, report: { columns: ['id', 'uuid'] } }));
    expect(s).toMatchObject({ uda: 'different', udaType: 'string', report: 'different', enabled: false });
    expect(s.columns).toEqual(['id', 'uuid']);
  });
});

describe('setupLines', () => {
  it('writes everything tasksh does, with the columns before the rest of the report', () => {
    expect(setupLines(config(), WANT)).toEqual([
      ['uda.reviewed.type', 'date'],
      ['uda.reviewed.label', 'Reviewed'],
      ['report._reviewed.columns', 'uuid'],
      ['report._reviewed.description', 'Tasksh review report.  Adjust the filter to your needs.'],
      ['report._reviewed.sort', 'reviewed+,modified+'],
      ['report._reviewed.filter', '( reviewed.none: or reviewed.before:now-6days ) and ( +PENDING or +WAITING )'],
    ]);
  });

  it('writes only what is missing', () => {
    const uda = { type: 'date', label: 'Reviewed' };
    expect(setupLines(config({ uda }), WANT).map(([n]) => n)).toEqual([
      'report._reviewed.columns',
      'report._reviewed.description',
      'report._reviewed.sort',
      'report._reviewed.filter',
    ]);
    expect(setupLines(config({ report: {} }), WANT)).toEqual([
      ['uda.reviewed.type', 'date'],
      ['uda.reviewed.label', 'Reviewed'],
    ]);
    expect(setupLines(config({ uda, report: {} }), WANT)).toEqual([]);
  });

  it('changes a UDA of another type but keeps a label it has, and rewrites a report with other columns', () => {
    const lines = setupLines(config({ uda: { type: 'string', label: 'When' }, report: { columns: ['id'] } }), WANT);
    expect(lines[0]).toEqual(['uda.reviewed.type', 'date']);
    expect(lines.map(([n]) => n)).not.toContain('uda.reviewed.label');
    expect(lines.map(([n]) => n)).toEqual([
      'uda.reviewed.type',
      'report._reviewed.columns',
      'report._reviewed.sort',
      'report._reviewed.filter',
    ]);
  });

  it('applies an adjusted filter or sort and nothing else', () => {
    const uda = { type: 'date', label: 'Reviewed' };
    const want = { filter: 'reviewed.before:now-30days and +PENDING', sort: REVIEW_SORT };
    expect(setupLines(config({ uda, report: {} }), want)).toEqual([['report._reviewed.filter', want.filter]]);
    expect(setupLines(config({ uda, report: {} }), { ...WANT, sort: 'urgency-' })).toEqual([
      ['report._reviewed.sort', 'urgency-'],
    ]);
  });

  it('only ever names the reviewed UDA and the _reviewed report', () => {
    for (const [name] of setupLines(config(), WANT)) expect(name).toMatch(/^(uda\.reviewed\.|report\._reviewed\.)/);
  });
});

describe('configArgs', () => {
  it('asks no questions, and keeps a value with spaces as one word', () => {
    expect(configArgs(['report._reviewed.filter', 'a and b'])).toEqual([
      'rc.confirmation:no',
      'rc.verbose:nothing',
      'config',
      'report._reviewed.filter',
      'a and b',
    ]);
  });
});

describe('parseReview', () => {
  it('reads the command as tasksh does: cut to three letters, with a count, after task', () => {
    for (const line of ['review', 'task review', 'rev', 'revi', 'REVIEW', '  review  '])
      expect(parseReview(line), line).toEqual({ limit: 0 });
    expect(parseReview('review 10')).toEqual({ limit: 10 });
    expect(parseReview('task rev 3')).toEqual({ limit: 3 });
  });

  it('leaves everything else to Taskwarrior', () => {
    for (const line of [
      '',
      're',
      'reviews',
      'review foo',
      'review 1 2',
      'review 3x',
      'revue',
      'next',
      'add review the code',
      'project:Work review',
    ])
      expect(parseReview(line), line).toBeNull();
  });
});

describe('menuKey', () => {
  it('knows the choices of the review menu, and Enter is the usual one', () => {
    expect(['', 'r', 's', 'e', 'm', 'c', 'd', 'q'].map(menuKey)).toEqual([
      'reviewed',
      'reviewed',
      'skip',
      'edit',
      'modify',
      'complete',
      'delete',
      'quit',
    ]);
    expect(menuKey('  S ')).toBe('skip');
  });

  it('does not know any other word', () => {
    for (const w of ['x', 'review', 'yes', 'ss', '1']) expect(menuKey(w), w).toBeNull();
  });
});

describe('the setup questions', () => {
  it("are tasksh's own when something is missing", () => {
    const s = taskshStatus(config());
    expect(setupQuestion('uda', s)).toBe(
      "Tasksh needs to define a 'reviewed' UDA of type 'date' for all tasks.  Ok to proceed?",
    );
    expect(setupQuestion('report', s)).toBe(
      "Tasksh needs to define a '_reviewed' report to identify tasks needing review.  Ok to proceed?",
    );
  });

  it('say what is there when it is not the way tasksh needs it', () => {
    const s = taskshStatus(config({ uda: { type: 'string', label: null }, report: { columns: ['id', 'uuid'] } }));
    expect(setupQuestion('uda', s)).toContain("of type 'string' now");
    expect(setupQuestion('report', s)).toContain("columns 'id, uuid' now");
  });

  it('write each part on its own', () => {
    const lines = setupLines(config(), WANT);
    expect(partLines(lines, 'uda').map(([n]) => n)).toEqual(['uda.reviewed.type', 'uda.reviewed.label']);
    expect(partLines(lines, 'report').map(([n]) => n)).toEqual([
      'report._reviewed.columns',
      'report._reviewed.description',
      'report._reviewed.sort',
      'report._reviewed.filter',
    ]);
  });
});
