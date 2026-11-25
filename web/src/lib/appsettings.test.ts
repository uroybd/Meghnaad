import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import {
  emptyAppearance,
  nameProblem,
  sameAppearance,
  parseAppearance,
  progressFraction,
  serializeAppearance,
  commandLabel,
  commandProblem,
  parseHomepage,
  parseSettings,
  serializeBoards,
  serializeHomepage,
  serializeSettings,
  lintSettings,
  mergeSettings,
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

describe('aliasIcons', () => {
  it('reads an icon for each alias, and says what it cannot use', () => {
    const p = parseSettings(JSON.stringify({ aliasIcons: { rem: 'bell', bad: 'Not An Icon', n: 3 } }));
    expect(p.aliasIcons).toEqual({ rem: 'bell' });
    expect(p.errors).toEqual([
      'aliasIcons.bad: must be a Lucide icon name such as "bell"',
      'aliasIcons.n: must be a Lucide icon name such as "bell"',
    ]);
    expect(parseSettings('{ "aliasIcons": [] }').errors).toEqual([
      'aliasIcons must be an object: { "<alias>": "<icon>" }',
    ]);
    expect(lintSettings('{ "aliasIcons": [] }').errors).toEqual([
      'aliasIcons must be an object: { "<alias>": "<icon>" }',
    ]);
  });

  it('saves the icons as given, keeping one it could not read and everything else', () => {
    const raw = { aliasIcons: { rem: 'bell', bad: 7, gone: 'x' }, taskCommands: [{ name: 'a', command: 'b' }] };
    const cmds = [{ name: 'a', command: 'b' }];
    expect(JSON.parse(serializeSettings(raw, cmds, { rem: 'alarm-clock', gone: 'x' }))).toEqual({
      aliasIcons: { rem: 'alarm-clock', bad: 7, gone: 'x' },
      taskCommands: [{ name: 'a', command: 'b' }],
    });
    // Without icons given, the saved ones stay as they are.
    expect(JSON.parse(serializeSettings(raw, [])).aliasIcons).toEqual(raw.aliasIcons);
    // The last one taken out: the section goes (all but what it could not read).
    expect(JSON.parse(serializeSettings({ aliasIcons: { rem: 'bell' } }, [], {}))).toEqual({});
  });
});

describe('a task command’s label', () => {
  it('is read, saved and shown in the menus; without one the name is made words', () => {
    const p = parseSettings(
      '{ "taskCommands": [ { "name": "kanban_next", "command": "modify +next", "label": "Move to next column" }, { "name": "follow-up.later", "command": "wait:tomorrow" }, { "name": "odd", "command": "done", "label": 3 } ] }',
    );
    expect(p.commands[0]).toEqual({ name: 'kanban_next', command: 'modify +next', label: 'Move to next column' });
    expect(p.commands.map(commandLabel)).toEqual(['Move to next column', 'Follow up later', 'Odd']);
    // A label that isn't text is said, and the command is kept without it.
    expect(p.errors).toEqual(['taskCommands[2] (odd): label must be text']);
    expect(p.commands[2]).toEqual({ name: 'odd', command: 'done' });
    expect(commandLabel({ name: 'kanban__next', label: '  ' })).toBe('Kanban next');
    const saved = JSON.parse(serializeSettings(p.raw, p.commands));
    expect(saved.taskCommands[0].label).toBe('Move to next column');
    expect(saved.taskCommands[1]).toEqual({ name: 'follow-up.later', command: 'wait:tomorrow' });
  });
});

describe('a task command’s name', () => {
  it('can’t be a command already: tasksh’s review, Taskwarrior’s, a hidden one, the console’s own', () => {
    for (const name of ['review', 'modify', '_get', 'burndown.daily', 'next', 'clear'])
      expect(nameProblem(name), name).toMatch(/is a command already/);
    expect(nameProblem('review')).toBe(
      '‘review’ is a command already: give it another name (its label can still say “Review”)',
    );
    // An abbreviation `task` would run, and tasksh's from three letters.
    expect(nameProblem('sync')).toBe(
      '‘sync’ runs the command synchronize: give it another name (its label can still say “Sync”)',
    );
    expect(nameProblem('rev')).toMatch(/runs the command review/);
    for (const name of ['inreview', 'inprogress', 'pending', 'fail', 'standup'])
      expect(nameProblem(name), name).toBeNull();
    // The taskrc's own reports, when the settings panel has them.
    const config = { reports: { standup: { name: 'standup' } } } as never;
    expect(nameProblem('standup', config)).toMatch(/is a command already/);
    const p = parseSettings('{ "taskCommands": [ { "name": "review", "command": "modify +r", "label": "Review" } ] }');
    expect(p.commands).toEqual([]);
    expect(p.errors).toEqual([
      'taskCommands[0] (review): ‘review’ is a command already: give it another name (its label can still say “Review”)',
    ]);
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

  it('leaves the key out when there are none, keeps what it could not read, and reads back what it wrote', () => {
    expect(JSON.parse(serializeSettings({ taskCommands: [{ name: 'a', command: 'b' }] }, []))).toEqual({});
    expect(JSON.parse(serializeSettings({ taskCommands: [{}] }, []))).toEqual({ taskCommands: [{}] });
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
  it('is a report, a board or a kanban, one of the three', () => {
    expect(parseHomepage({ report: ' list ' })).toEqual({ report: 'list' });
    expect(parseHomepage({ board: 'week' })).toEqual({ board: 'week' });
    expect(parseHomepage({ kanban: 'work' })).toEqual({ kanban: 'work' });
    for (const bad of [
      undefined,
      null,
      'next',
      [],
      {},
      { report: '' },
      { report: 3 },
      { report: 'a', board: 'b' },
      { kanban: 'a', board: 'b' },
      { kanban: ' ' },
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

describe('appearance', () => {
  it('reads the UDAs shown as progress bars, with a default range', () => {
    const p = parseSettings(
      '{ "appearance": { "progress": { "progress": { "min": 0, "max": 10 }, "done": {}, "pct": { "max": 1.5 } } } }',
    );
    expect(p.appearanceErrors).toEqual([]);
    expect(p.appearance.progress).toEqual({
      progress: { min: 0, max: 10 },
      done: { min: 0, max: 100 },
      pct: { min: 0, max: 1.5 },
    });
  });

  it('says what is wrong and keeps what is right', () => {
    const { appearance, errors } = parseAppearance({
      progress: {
        ok: { min: 1, max: 2 },
        flat: { min: 5, max: 5 },
        backwards: { min: 9, max: 1 },
        text: { max: '9' },
        no: 3,
      },
    });
    expect(Object.keys(appearance.progress)).toEqual(['ok']);
    expect(errors).toHaveLength(4);
    expect(errors.join('\n')).toContain('flat: min must be less than max');
    expect(parseAppearance('x').errors).toHaveLength(1);
    expect(parseAppearance({ progress: [] }).errors).toHaveLength(1);
    expect(parseAppearance(undefined)).toEqual({ appearance: emptyAppearance(), errors: [] });
    expect(parseAppearance({ idFallback: true }).appearance.idFallback).toBe(true);
    expect(parseAppearance({ idFallback: 'yes' }).errors).toEqual(['appearance.idFallback must be true or false']);
    expect(parseAppearance({}).appearance.cardIcons).toBe(false);
    expect(parseAppearance({ cardIcons: true }).appearance.cardIcons).toBe(true);
    expect(parseAppearance({ cardIcons: 'yes' }).errors).toEqual(['appearance.cardIcons must be true or false']);
    const on = serializeAppearance({}, { ...parseAppearance({}).appearance, cardIcons: true });
    expect(JSON.parse(on)).toEqual({ appearance: { cardIcons: true } });
  });

  it('is kept when the rest of the settings are saved, and saved without touching them', () => {
    const raw = { homepage: { report: 'list' }, taskCommands: [{ name: 'a', command: 'done' }], keep: 1 };
    const text = serializeAppearance(raw, {
      progress: { progress: { min: 0, max: 100 } },
      transformations: [],
      idFallback: false,
      cardIcons: false,
    });
    expect(JSON.parse(text)).toEqual({ ...raw, appearance: { progress: { progress: { min: 0, max: 100 } } } });
    // None left leaves the key out.
    expect(JSON.parse(serializeAppearance({ ...raw, appearance: { progress: {} } }, emptyAppearance()))).toEqual(raw);
    const kept = parseSettings(text);
    expect(serializeSettings(kept.raw, kept.commands)).toContain('"appearance"');
  });

  it('reads the transformations in order, keeps a bad pattern for the engine to report, and saves them', () => {
    const jira = { match: '\\b(BOR-\\d+)\\b', replace: '[$1](https://j.example/$1)', fields: ['description'] };
    const { appearance, errors } = parseAppearance({
      transformations: [jira, { match: '(', fields: ['jira'] }, { match: 'x' }, 'nope', { match: 1, fields: [] }],
    });
    expect(appearance.transformations).toEqual([jira, { match: '(', replace: '', fields: ['jira'] }]);
    expect(errors).toEqual([
      'appearance.transformations[2]: fields must be a list of field names',
      'appearance.transformations[3]: must be { "match": "…", "replace": "…", "fields": [ … ] }',
      'appearance.transformations[4]: must be { "match": "…", "replace": "…", "fields": [ … ] }',
    ]);
    expect(parseAppearance({ transformations: {} }).errors).toHaveLength(1);
    const text = serializeAppearance(
      {},
      { progress: {}, transformations: [jira], idFallback: false, cardIcons: false },
    );
    expect(JSON.parse(text)).toEqual({ appearance: { transformations: [jira] } });
    expect(parseSettings(text).appearance.transformations).toEqual([jira]);
    expect(sameAppearance(appearance, { ...appearance, transformations: [jira] })).toBe(false);
    expect(sameAppearance(appearance, structuredClone(appearance))).toBe(true);
  });

  it("reads a transformation's condition, trimmed, and saves it; an empty one is none", () => {
    const rule = { match: 'x', replace: 'y', fields: ['description'] };
    const { appearance, errors } = parseAppearance({
      transformations: [
        { ...rule, condition: ' project:FUL ' },
        { ...rule, condition: '  ' },
        { ...rule, condition: 3 },
      ],
    });
    expect(appearance.transformations).toEqual([{ ...rule, condition: 'project:FUL' }, rule]);
    expect(errors).toEqual(['appearance.transformations[2]: condition must be a Taskwarrior filter, as text']);
    const text = serializeAppearance({}, { ...appearance, idFallback: false, cardIcons: false });
    expect(JSON.parse(text).appearance.transformations[0]).toEqual({ ...rule, condition: 'project:FUL' });
  });

  it('fills a bar from empty at the min to full at the max', () => {
    const r = { min: 0, max: 50 };
    expect(progressFraction(25, r)).toBe(0.5);
    expect(progressFraction(-5, r)).toBe(0);
    expect(progressFraction(80, r)).toBe(1);
    expect(progressFraction(Number.NaN, r)).toBe(0);
  });
});

// Spec 0009: the cases every front end's merge is tested against, and a file with what no front end knows at every depth.
const kept = JSON.parse(readFileSync('../fixtures/settings-kept.json', 'utf8')) as {
  merge: { name: string; old: unknown; base: unknown; next: unknown; expect: unknown }[];
  file: Record<string, unknown>;
  kept: (string | number)[][];
};
const at = (v: unknown, path: (string | number)[]) =>
  path.reduce<unknown>((o, k) => (o as Record<string | number, unknown> | undefined)?.[k], v);

describe('saving keeps what this app does not use', () => {
  for (const c of kept.merge)
    it(`merges: ${c.name}`, () => {
      expect(mergeSettings(c.old, c.base, c.next)).toEqual(c.expect);
    });

  it('keeps every unknown key and unusable entry through each kind of save', () => {
    const read = parseSettings(JSON.stringify(kept.file));
    expect(read.errors).toHaveLength(1); // the task command named “two words”
    expect(read.boardErrors).toHaveLength(1); // the board with a bad id
    const saves = {
      commands: serializeSettings(read.raw, [...read.commands, { name: 'late', command: '+OVERDUE list' }]),
      'commands, one edited': serializeSettings(read.raw, [{ name: 'fail', command: 'done outcome:failed' }]),
      boards: serializeBoards(read.raw, read.boards),
      'boards, one renamed': serializeBoards(read.raw, [{ ...read.boards[0], name: 'This week' }]),
      homepage: serializeHomepage(read.raw, { board: 'week' }),
      appearance: serializeAppearance(read.raw, {
        ...read.appearance,
        progress: { ...read.appearance.progress, pct: { min: 0, max: 1 } },
      }),
      'appearance, cleared': serializeAppearance(read.raw, emptyAppearance()),
    };
    for (const [what, text] of Object.entries(saves)) {
      const out = JSON.parse(text) as unknown;
      for (const path of kept.kept) {
        // Clearing the appearance takes the transformation it understood, with what was in it.
        if (what === 'appearance, cleared' && path[1] === 'transformations') continue;
        expect(at(out, path), `${what}: ${path.join('.')}`).toEqual(at(kept.file, path));
      }
    }
    expect(at(JSON.parse(saves['commands, one edited']), ['taskCommands', 0])).toEqual({
      name: 'fail',
      command: 'done outcome:failed',
      'x-kept': 2,
    });
    expect(JSON.parse(saves['appearance, cleared']).appearance).toEqual({ 'x-kept': true });
  });

  it('saves the ID fallback with the rest of the appearance, and takes it out when it is turned off', () => {
    const raw = { appearance: { progress: { done: { min: 0, max: 10 } }, mine: 1 }, other: 2 };
    const on = JSON.parse(
      serializeAppearance(raw, { ...parseSettings(JSON.stringify(raw)).appearance, idFallback: true }),
    );
    expect(on).toEqual({
      appearance: { progress: { done: { min: 0, max: 10 } }, mine: 1, idFallback: true },
      other: 2,
    });
    const read = parseSettings(JSON.stringify(on));
    expect(read.appearance.idFallback).toBe(true);
    expect(JSON.parse(serializeAppearance(on, { ...read.appearance, idFallback: false }))).toEqual(raw);
  });

  it('writes a board read from its date ranges with its controls alone', () => {
    const legacy = {
      id: 'week',
      name: 'Week',
      ranges: [{ name: 'when', default: 'this week' }],
      sections: [{ id: 'open', type: 'number', x: 0, y: 0, w: 3, h: 2 }],
    };
    const read = parseSettings(JSON.stringify({ boards: [legacy] }));
    const out = JSON.parse(serializeBoards(read.raw, read.boards));
    expect(out.boards[0].ranges).toBeUndefined();
    expect(parseSettings(JSON.stringify(out)).boardErrors).toEqual([]);
  });
});

describe('lintSettings', () => {
  it('takes a device’s "homepage": null (next), and no other plain value', () => {
    expect(lintSettings('{ "homepage": null }').errors).toEqual([]);
    expect(lintSettings('{ "homepage": "next" }').errors).toEqual(['homepage must be an object']);
  });
  it('stops at what is not settings', () => {
    expect(lintSettings('{ nope').errors[0]).toMatch(/Not valid JSON/);
    expect(lintSettings('[1]').errors[0]).toMatch(/must be a JSON object/);
    expect(lintSettings(JSON.stringify({ x: 'a'.repeat(70 * 1024) })).errors[0]).toMatch(/at most 64 KB/);
    expect(lintSettings('')).toEqual({ errors: [], warnings: [] });
  });

  it('calls a known key in the wrong shape an error', () => {
    const { errors } = lintSettings(
      JSON.stringify({
        homepage: 'next',
        appearance: { progress: [], transformations: {} },
        taskCommands: {},
        boards: [3, { id: 'week', name: 'Week' }],
        tui: [],
      }),
    );
    expect(errors).toEqual([
      'homepage must be an object',
      'tui must be an object',
      'taskCommands must be a list',
      expect.stringMatching(/^appearance.progress must be an object/),
      expect.stringMatching(/^appearance.transformations must be a list/),
      'boards[0] must be an object',
      'boards[1] (week): needs a sections list',
    ]);
  });

  it('calls what this app can’t use a warning, with where it is', () => {
    const { errors, warnings } = lintSettings(JSON.stringify(kept.file), () => ['keys: no action is called app.nope']);
    expect(errors).toEqual([]);
    expect(warnings).toEqual([
      'taskCommands[1] (two words): a name is one word (no spaces)',
      "board 'Not An Id': needs an id of lower case letters, digits, - and _",
      'keys: no action is called app.nope',
    ]);
  });
});
