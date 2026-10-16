import { describe, expect, it } from 'vitest';
import {
  nameProblem,
  commandProblem,
  parseHomepage,
  parseSettings,
  serializeBoards,
  serializeHomepage,
  serializeSettings,
} from './appsettings';

describe('parseSettings', () => {
  it('reads the task commands', () => {
    const p = parseSettings(
      '{ "taskCommands": [ { "name": "fail", "command": "done outcome:fail", "icon": "circle-x" }, { "name": "late", "command": " +OVERDUE list " } ] }',
    );
    expect(p.errors).toEqual([]);
    expect(p.commands).toEqual([
      { name: 'fail', command: 'done outcome:fail', icon: 'circle-x' },
      { name: 'late', command: '+OVERDUE list' },
    ]);
  });

  it('takes nothing, or an object with no commands, as no commands', () => {
    for (const t of ['', '  \n', '{}', '{ "other": 1 }'])
      expect(parseSettings(t)).toMatchObject({ commands: [], errors: [] });
  });

  it('says what is wrong, and keeps what is right', () => {
    const p = parseSettings(
      JSON.stringify({
        taskCommands: [
          { name: 'ok', command: 'done' },
          { name: 'two words', command: 'done' },
          { name: 'a:b', command: 'done' },
          { name: '12', command: 'done' },
          { name: 'nocmd' },
          { name: 'badicon', command: 'done', icon: 'Circle X' },
          { name: 'ok', command: 'stop' },
          'text',
          { command: 'done' },
        ],
      }),
    );
    expect(p.commands).toEqual([{ name: 'ok', command: 'done' }]);
    expect(p.errors).toHaveLength(8);
    expect(p.errors.join('\n')).toMatch(/two words.*one word/);
    expect(p.errors.join('\n')).toMatch(/needs a command/);
    expect(p.errors.join('\n')).toMatch(/Lucide icon name/);
    expect(p.errors.join('\n')).toMatch(/already a command named ok/);
  });

  it('refuses what is not a JSON object', () => {
    expect(parseSettings('{ taskCommands: [] }').errors[0]).toMatch(/Not valid JSON/);
    expect(parseSettings('[]').errors[0]).toMatch(/must be a JSON object/);
    expect(parseSettings('{ "taskCommands": 3 }').errors[0]).toMatch(/must be a list/);
  });
});

describe('serializeSettings', () => {
  it('writes the commands and keeps every other key', () => {
    const p = parseSettings('{ "future": { "a": 1 }, "taskCommands": [ { "name": "x", "command": "done" } ] }');
    const text = serializeSettings(p.raw, [{ name: 'fail', command: 'done outcome:fail', icon: 'circle-x' }]);
    expect(JSON.parse(text)).toEqual({
      future: { a: 1 },
      taskCommands: [{ name: 'fail', command: 'done outcome:fail', icon: 'circle-x' }],
    });
    expect(text.endsWith('}\n')).toBe(true);
  });

  it('leaves the key out when there are none, and reads back what it wrote', () => {
    expect(JSON.parse(serializeSettings({ taskCommands: [{}] }, []))).toEqual({});
    const cmds = [{ name: 'a', command: 'b c' }];
    expect(parseSettings(serializeSettings({}, cmds)).commands).toEqual(cmds);
  });
});

describe('nameProblem', () => {
  it('accepts a word and refuses what could be a filter', () => {
    for (const n of ['fail', 'done2', 'my_cmd', 'x.y']) expect(nameProblem(n), n).toBeNull();
    for (const n of ['', 'a b', '+x', '-x', 'a:b', 'a=b', '5', '1,2', '/x', 'task', '--'])
      expect(nameProblem(n), n).not.toBeNull();
  });
});

describe('a command that is several commands', () => {
  it('is read, and one with nothing on a side of an operator is reported and left out', () => {
    const p = parseSettings(
      '{ "taskCommands": [ { "name": "wrap", "command": "done && modify +failed" }, { "name": "bad", "command": "done &&" } ] }',
    );
    expect(p.commands).toEqual([{ name: 'wrap', command: 'done && modify +failed' }]);
    expect(p.errors).toEqual(['taskCommands[1] (bad): Nothing after ‘&&’.']);
    expect(commandProblem('done ; modify +x')).toBeNull();
    expect(commandProblem('|| done')).toBe('Nothing before ‘||’.');
    expect(commandProblem('  ')).toBe('needs a command');
  });
});

describe('boards in the settings', () => {
  const board = { id: 'week', name: 'Week', sections: [{ id: 'open', type: 'number', x: 0, y: 0, w: 3, h: 2 }] };

  it('are read and tidied by the engine, and their problems kept apart from the commands’', () => {
    const p = parseSettings(JSON.stringify({ boards: [board, { id: 'Bad Id', name: 'x' }] }));
    expect(p.boards.map((b) => b.id)).toEqual(['week']);
    expect(p.boards[0].sections[0].measure).toEqual({ fn: 'count' });
    expect(p.boardErrors).toHaveLength(1);
    // Saving the commands does not drop boards, so their problems are not "dropped on apply" warnings.
    expect(p.errors).toEqual([]);
  });

  it('are none in a file without them', () => {
    expect(parseSettings('{}')).toMatchObject({ boards: [], boardErrors: [] });
  });

  it('are saved beside the other keys, with a version, and leave the commands alone', () => {
    const raw = { taskCommands: [{ name: 'fail', command: 'done' }], other: 1 };
    const out = JSON.parse(serializeBoards(raw, parseSettings(JSON.stringify({ boards: [board] })).boards));
    expect(out).toMatchObject({ other: 1, boardsVersion: 1, taskCommands: [{ name: 'fail' }] });
    expect(out.boards).toHaveLength(1);
  });

  it('are left out of the file when there are none', () => {
    const out = JSON.parse(serializeBoards({ boards: [board], boardsVersion: 1, other: 1 }, []));
    expect(out).toEqual({ other: 1 });
  });

  it('survive saving the commands', () => {
    const raw = parseSettings(JSON.stringify({ boards: [board], boardsVersion: 1 })).raw;
    expect(JSON.parse(serializeSettings(raw, [{ name: 'fail', command: 'done' }]))).toMatchObject({
      boards: [{ id: 'week' }],
      boardsVersion: 1,
      taskCommands: [{ name: 'fail' }],
    });
  });
});

describe('the home page in the settings', () => {
  it('is a report or a board, one of the two', () => {
    expect(parseHomepage({ report: ' list ' })).toEqual({ report: 'list' });
    expect(parseHomepage({ board: 'week' })).toEqual({ board: 'week' });
    for (const bad of [
      undefined,
      null,
      'next',
      [],
      {},
      { report: '' },
      { report: 3 },
      { report: 'a', board: 'b' },
      { report: 'a', x: 1 },
    ])
      expect(parseHomepage(bad)).toBeNull();
  });

  it('is read with the rest of the file, and none is none', () => {
    expect(parseSettings('{ "homepage": { "board": "week" } }').homepage).toEqual({ board: 'week' });
    expect(parseSettings('{ "homepage": 7 }')).toMatchObject({ homepage: null, errors: [] });
    expect(parseSettings('').homepage).toBeNull();
  });

  it('is saved beside the other keys, and survives saving the commands and the boards', () => {
    const raw = { taskCommands: [{ name: 'fail', command: 'done' }], boards: [] };
    const out = JSON.parse(serializeHomepage(raw, { report: 'list' }));
    expect(out).toEqual({ ...raw, homepage: { report: 'list' } });
    expect(JSON.parse(serializeHomepage(out, null))).toEqual(raw);
    expect(JSON.parse(serializeSettings(out, [])).homepage).toEqual({ report: 'list' });
    expect(JSON.parse(serializeBoards(out, [])).homepage).toEqual({ report: 'list' });
  });
});
