import { describe, expect, it } from 'vitest';
import { lintSettings, parseSettings, serializeKanban } from './appsettings';
import {
  automaticColumns,
  automaticSettings,
  reportLines,
  columnArgs,
  columnOf,
  emptyKanbanSettings,
  hasReview,
  moveSteps,
  lanesOf,
  ONLY_THIS_TASK,
  parseKanban,
  newKanban,
  placeTasks,
  renamedKanban,
  resolveKanban,
  setupLines,
  udaNameProblem,
  udaStatus,
} from './kanban';
import type { ConfigResponse, Row, UdaDef } from './types';

const configWith = (udas: Record<string, Partial<UdaDef>>): ConfigResponse =>
  ({
    config: {
      udas: Object.fromEntries(
        Object.entries(udas).map(([name, u]) => [
          name,
          { name, type: 'string', label: null, values: [], default: null, indicator: null, ...u },
        ]),
      ),
    },
    reports: [],
  }) as unknown as ConfigResponse;

const row = (uuid: string) => ({ uuid }) as Row;

describe('the automatic setup', () => {
  const U = 'progressState';

  it('defines the UDA when it is missing', () => {
    expect(udaStatus(null, U).part).toBe('missing');
    expect(setupLines(configWith({}), U)).toEqual([
      ['uda.progressState.type', 'string'],
      ['uda.progressState.label', 'Progress'],
      ['uda.progressState.values', 'inprogress,review'],
    ]);
    expect(setupLines(configWith({}), 'stage')[0]).toEqual(['uda.stage.type', 'string']);
  });

  it('leaves alone a UDA that already fits, with any values or with ours among its own', () => {
    for (const values of [[], ['inprogress', 'review'], ['blocked', 'inprogress', 'review']]) {
      const c = configWith({ progressState: { label: 'P', values } });
      expect(udaStatus(c, U).part).toBe('ok');
      expect(setupLines(c, U)).toEqual([]);
    }
  });

  it('adds our values to the ones there, and fixes the type, keeping the label', () => {
    const c = configWith({ progressState: { label: 'Stage', values: ['blocked'] } });
    expect(udaStatus(c, U).part).toBe('different');
    expect(setupLines(c, U)).toEqual([['uda.progressState.values', 'blocked,inprogress,review']]);
    const d = configWith({ progressState: { type: 'numeric', label: 'Stage' } });
    expect(setupLines(d, U)).toEqual([
      ['uda.progressState.type', 'string'],
      ['uda.progressState.values', 'inprogress,review'],
    ]);
  });

  it('takes a UDA name Taskwarrior would', () => {
    expect(udaNameProblem('progressState')).toBeNull();
    expect(udaNameProblem('stage_2')).toBeNull();
    expect(udaNameProblem('')).toBe('needs a UDA name');
    expect(udaNameProblem('2stage')).toMatch(/starting with a letter/);
    expect(udaNameProblem('my stage')).toMatch(/letters, digits and _/);
    expect(udaNameProblem('project')).toMatch(/Taskwarrior's own/);
  });

  it('makes To do, In progress, Review and Done; Review clears reviewed when tasksh review is set up', () => {
    expect(automaticColumns(false)).toEqual([
      { name: 'To do', filter: 'status:pending progressState:', move: 'modify status:pending progressState:' },
      {
        name: 'In progress',
        filter: 'status:pending progressState:inprogress',
        move: 'modify status:pending progressState:inprogress',
      },
      {
        name: 'Review',
        filter: 'status:pending progressState:review',
        move: 'modify status:pending progressState:review',
      },
      { name: 'Done', filter: 'status:completed', move: 'done' },
    ]);
    expect(automaticColumns(true)[2].move).toBe('modify status:pending progressState:review reviewed:');
    expect(automaticColumns(false, 'stage')[1].filter).toBe('status:pending stage:inprogress');
    expect(hasReview(configWith({ reviewed: { type: 'date' } }))).toBe(true);
    expect(hasReview(configWith({}))).toBe(false);
  });

  it('turns Kanban on with its settings on the UDA and a first kanban of no settings of its own', () => {
    const s = automaticSettings(emptyKanbanSettings(), false, 'stage');
    expect(s).toMatchObject({
      enabled: true,
      setup: 'automatic',
      uda: 'stage',
      report: 'all',
      filter: '( +PENDING or +COMPLETED )',
      completedSince: 'today-14d',
      kanbans: [{ id: 'kanban', name: 'Kanban' }],
    });
    expect(s.columns.map((c) => c.name)).toEqual(['To do', 'In progress', 'Review', 'Done']);
    // Nothing starts a task: started is time tracking, not a stage.
    expect(s.columns.some((c) => /\bstart\b/.test(c.move))).toBe(false);
    // Kanbans already there are kept as they are.
    const mine = { id: 'mine', name: 'Mine', report: 'list' };
    expect(automaticSettings({ ...emptyKanbanSettings(), kanbans: [mine] }, false, U).kanbans).toEqual([mine]);
  });

  it('can draw them on a kanban report of their own, added to the taskrc unless there is one', () => {
    const s = automaticSettings(emptyKanbanSettings(), false, U, true);
    expect(s).toMatchObject({ report: 'kanban', filter: '', completedSince: 'today-14d' });
    const lines = reportLines(configWith({}));
    expect(lines.map(([n]) => n)).toEqual([
      'report.kanban.description',
      'report.kanban.columns',
      'report.kanban.labels',
      'report.kanban.filter',
      'report.kanban.sort',
    ]);
    expect(Object.fromEntries(lines)['report.kanban.filter']).toBe('( +PENDING or +COMPLETED )');
    const withOne = configWith({});
    withOne.config.reports = { kanban: {} };
    expect(reportLines(withOne)).toEqual([]);
  });
});

describe('reading a kanban', () => {
  const view = { report: 'all', filter: '( +PENDING or +COMPLETED )', completedSince: 'today-14d' };

  it('brackets each filter so an or in one stays in it, then names the report', () => {
    expect(columnArgs(view, { name: 'To do', filter: 'status:pending progressState:', move: '' })).toEqual([
      '(',
      '(',
      '+PENDING',
      'or',
      '+COMPLETED',
      ')',
      ')',
      '(',
      '-COMPLETED',
      'or',
      'end.after:today-14d',
      ')',
      '(',
      'status:pending',
      'progressState:',
      ')',
      'all',
    ]);
    expect(columnArgs({ report: 'next', filter: '', completedSince: '' }, null)).toEqual(['next']);
    expect(columnArgs({ report: 'list', filter: 'project:Home', completedSince: '' }, null)).toEqual([
      '(',
      'project:Home',
      ')',
      'list',
    ]);
  });

  it('puts a task in the first column that has it, and the rest the kanban picks in Other', () => {
    const placed = placeTasks(
      [[row('a'), row('b')], [row('b'), row('c')], []],
      [row('a'), row('b'), row('c'), row('d')],
    );
    expect(placed.columns.map((c) => c.map((r) => r.uuid))).toEqual([['a', 'b'], ['c'], []]);
    expect(placed.other.map((r) => r.uuid)).toEqual(['d']);
    expect(columnOf(placed.columns, 'c')).toBe(1);
    expect(columnOf(placed.columns, 'd')).toBe(-1);
  });
});

describe('moving a card', () => {
  const uuid = '1a2b3c4d-0000-4000-8000-000000000000';

  it('runs the line on that task alone, the uuid in front of each statement (a recurring one without asking)', () => {
    expect(moveSteps('done', uuid, [])).toEqual([{ args: [ONLY_THIS_TASK, uuid, 'done'], join: null }]);
    expect(moveSteps('modify progressState:review ; annotate "in review"', uuid, [])).toEqual([
      { args: [ONLY_THIS_TASK, uuid, 'modify', 'progressState:review'], join: null },
      { args: [ONLY_THIS_TASK, uuid, 'annotate', 'in review'], join: ';' },
    ]);
  });

  it('writes out a task command, which keeps the task in front of each of its statements', () => {
    const commands = [{ name: 'ship', command: 'modify +shipped && done' }];
    expect(moveSteps('ship', uuid, commands)).toEqual([
      { args: [ONLY_THIS_TASK, uuid, 'modify', '+shipped'], join: null },
      { args: [ONLY_THIS_TASK, uuid, 'done'], join: '&&' },
    ]);
  });
});

describe('settings.json', () => {
  const columns = [{ name: 'Doing', filter: '+doing', move: 'modify +doing' }];
  const file = {
    enabled: true,
    setup: 'manual',
    uda: 'stage',
    report: 'all',
    filter: '',
    completedSince: 'today-14d',
    columns,
    kanbans: [{ id: 'work', name: 'Work', filter: 'project:Work' }],
  };

  it("draws a kanban with its own settings, else Kanban's", () => {
    const { kanban } = parseKanban(file);
    const own = { id: 'mine', name: 'Mine', report: 'list', completedSince: '', columns: [] };
    expect(resolveKanban(kanban, kanban.kanbans[0])).toEqual({
      id: 'work',
      name: 'Work',
      report: 'all',
      filter: 'project:Work',
      completedSince: 'today-14d',
      columns,
    });
    expect(resolveKanban(kanban, own)).toEqual({ ...own, filter: '' });
  });

  it('is read, the kanbans with a problem left out and said', () => {
    const work = { id: 'work', name: 'Work' };
    const { kanban: k, errors } = parseKanban({
      ...file,
      kanbans: [
        work,
        { ...work, id: 'Bad Slug' },
        { ...work },
        { ...work, id: 'two', columns: [{ name: '', filter: 'x', move: '' }] },
        { ...work, id: 'three', columns: [{ name: 'A', filter: 'x', move: 'done &&' }] },
        { id: 'four', name: 'Four', report: '' },
        { ...work, id: 'five', filter: 3 },
        'nope',
      ],
    });
    expect(k).toEqual({ ...file, kanbans: [work] });
    expect(errors).toHaveLength(7);
    expect(errors[1]).toContain('there is already a kanban at /kanban/work');
    expect(errors[2]).toContain('column 1: a column needs a name');
    expect(errors[4]).toContain("needs a report, its own or Kanban's");
    expect(errors[5]).toBe('kanban.kanbans[6] (five): filter must be text');
    expect(parseKanban(undefined)).toEqual({ kanban: emptyKanbanSettings(), errors: [] });
  });

  it("says what is wrong with the UDA and Kanban's columns, and uses none of the columns then", () => {
    const { kanban, errors } = parseKanban({ uda: 'project', columns: [...columns, { name: 'B', filter: '' }] });
    expect(kanban.uda).toBe('progressState');
    expect(kanban.columns).toEqual([]);
    expect(errors).toEqual([
      "kanban.uda: project is one of Taskwarrior's own attributes",
      'kanban.columns: column 2 (B): a column needs a filter',
    ]);
  });

  it('is saved, everything else and what this app does not know kept', () => {
    const raw = {
      taskCommands: [{ name: 'x', command: 'done' }],
      kanban: { ...file, later: 1, kanbans: [{ ...file.kanbans[0], colour: 'red' }, { id: 'Bad' }] },
    };
    const parsed = parseSettings(JSON.stringify(raw));
    expect(parsed.kanban.kanbans).toEqual(file.kanbans);
    const next = { ...parsed.kanban, report: 'list', kanbans: [{ id: 'work', name: 'Work 2', report: 'next' }] };
    const saved = JSON.parse(serializeKanban(parsed.raw, next));
    expect(saved.taskCommands).toEqual(raw.taskCommands);
    expect(saved.kanban.later).toBe(1);
    expect(saved.kanban.report).toBe('list');
    // Its own filter taken away, its own report given.
    expect(saved.kanban.kanbans).toEqual([
      { id: 'work', name: 'Work 2', report: 'next', colour: 'red' },
      { id: 'Bad' },
    ]);
  });

  it('writes nothing for a file without kanbans that stays without', () => {
    const parsed = parseSettings('{"taskCommands": []}');
    expect(JSON.parse(serializeKanban(parsed.raw, parsed.kanban))).toEqual({ taskCommands: [] });
  });

  it('lints the shape as an error and a bad kanban as a warning', () => {
    expect(lintSettings('{"kanban": []}').errors).toEqual(['kanban must be an object']);
    expect(lintSettings('{"kanban": {"kanbans": {}}}').errors).toEqual(['kanban.kanbans must be a list']);
    expect(lintSettings('{"kanban": {"columns": {}}}').errors).toEqual(['kanban.columns must be a list']);
    const l = lintSettings('{"kanban": {"report": "", "kanbans": [{"id": "a", "name": "A"}]}}');
    expect(l.errors).toEqual([]);
    expect(l.warnings).toEqual(["kanban.kanbans[0] (a): needs a report, its own or Kanban's"]);
  });
});

describe('naming a new kanban', () => {
  it('moves its address with the name until the address is typed or the kanban saved', () => {
    const fresh = newKanban('', ['kanban']);
    expect(fresh.id).toBe('kanban-2');
    let k = renamedKanban(fresh, 'W', false, ['kanban']);
    k = renamedKanban(k, 'Work', false, ['kanban']);
    expect(k).toEqual({ id: 'work', name: 'Work' });
    expect(renamedKanban({ id: 'mine', name: 'Work' }, 'Work 2', false, ['kanban']).id).toBe('mine');
    expect(renamedKanban({ id: 'work', name: 'Work' }, 'Jobs', true, ['kanban']).id).toBe('work');
    expect(renamedKanban(fresh, 'Kanban', false, ['kanban']).id).toBe('kanban-2');
  });
});

describe('swimlanes', () => {
  const row = (uuid: string) => ({ uuid }) as Row;

  it('has none when the report has no groups', () => {
    expect(lanesOf([row('a'), row('b')], [])).toEqual({ lanes: [], laneOf: new Map() });
  });

  it('makes a lane of each group heading, in the report’s order, a heading met again being the same lane', () => {
    const all = ['a', 'b', 'c', 'd', 'e'].map(row);
    const { lanes, laneOf } = lanesOf(all, [
      'Project: Home',
      null,
      'Project: Work',
      'Project: Home',
      'Project: (none)',
    ]);
    expect(lanes).toEqual(['Project: Home', 'Project: Work', 'Project: (none)']);
    expect([...laneOf.entries()]).toEqual([
      ['a', 0],
      ['b', 0],
      ['c', 1],
      ['d', 0],
      ['e', 2],
    ]);
  });
});
