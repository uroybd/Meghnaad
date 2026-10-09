import { describe, expect, it } from 'vitest';
import { apply, complete, insert, type Vocab } from './completion';

const vocab: Vocab = {
  contexts: ['work', 'home'],
  reports: [{ name: 'next', description: 'Most urgent' }, { name: 'list' }, { name: 'mine', description: 'My tasks' }],
  tasks: [
    { id: 3, uuid: 'aaaa1111-0000-0000-0000-000000000000', description: 'Fix the tap' },
    { id: 30, uuid: 'bbbb2222-0000-0000-0000-000000000000', description: 'Write report' },
    { id: null, uuid: 'cccc3333-0000-0000-0000-000000000000', description: 'Old done task' },
  ],
  projects: ['Home', 'Home.Kitchen', 'Work'],
  tags: ['errand', 'work', 'waiting-on'],
  udas: [
    { name: 'estimate', type: 'string', label: 'Size', values: ['big', 'small', ''], default: null, indicator: null },
    { name: 'ship', type: 'date', label: null, values: [], default: null, indicator: null },
  ],
};

const c = (line: string, caret = line.length) => {
  const r = complete(line, caret, vocab);
  // Compare option *values* in most tests; hints are checked separately.
  return { ...r, options: r.options.map((o) => o.value), raw: r };
};

describe('complete', () => {
  it('commands and reports by prefix', () => {
    expect(c('li').options).toEqual(['limit:', 'list']);
    expect(c('lis').options).toEqual(['list']);
    expect(c('mo').options).toEqual(['modify']);
    expect(c('mi').options).toEqual(['mine']);
    expect(c('3 do').options).toEqual(['done']);
  });

  it('attribute names get a colon, and share prefixes with commands', () => {
    expect(c('proj').options).toEqual(['project:', 'projects']);
    expect(c('est').options).toEqual(['estimate:']);
  });

  it('project values', () => {
    expect(c('project:Ho').options).toEqual(['project:Home', 'project:Home.Kitchen']);
    expect(c('project:Ho').common).toBe('project:Home');
    expect(c('project.is:W').options).toEqual(['project.is:Work']);
  });

  it('priority, status, UDA values, and date words', () => {
    expect(c('priority:').options).toEqual(['priority:H', 'priority:L', 'priority:M']);
    expect(c('priority:H').options).toEqual(['priority:H']);
    expect(c('estimate:b').options).toEqual(['estimate:big']);
    expect(c('estimate:').options).toEqual(['estimate:big', 'estimate:small']); // blank value omitted
    expect(c('due:tom').options).toEqual(['due:tomorrow']);
    expect(c('due.before:e').options).toEqual(['due.before:eod', 'due.before:eom', 'due.before:eow', 'due.before:eoy']);
    expect(c('ship:to').options).toEqual(['ship:today', 'ship:tomorrow']);
    expect(c('status:wa').options).toEqual(['status:waiting']);
  });

  it('tags and virtual tags', () => {
    expect(c('+w').options).toEqual(['+WAITING', '+WEEK', '+waiting-on', '+work']);
    expect(c('-err').options).toEqual(['-errand']);
    expect(c('+OVER').options).toEqual(['+OVERDUE']);
  });

  it('completes the word at the caret, not the whole line', () => {
    const line = 'project:Home +er due:today';
    const r = complete(line, '(project:Home +er'.length - 1, vocab);
    expect(r.options.map((o) => o.value)).toEqual(['+errand']);
    expect(r.start).toBe('project:Home '.length);
  });

  it('nothing for an empty word', () => {
    expect(c('list ').options).toEqual([]);
  });
});

describe('apply', () => {
  it('a unique match is completed with a trailing space', () => {
    const r = apply('3 do', complete('3 do', 4, vocab));
    expect(r).toMatchObject({ line: '3 done ', caret: 7, progressed: true });
  });

  it('an attribute completes up to the colon so the value can follow (no space)', () => {
    expect(apply('est', complete('est', 3, vocab)).line).toBe('estimate:');
  });

  it('a stem shared by an attribute and a command takes the common prefix', () => {
    expect(apply('proj', complete('proj', 4, vocab)).line).toBe('project');
  });

  it('ambiguous matches take the common prefix only', () => {
    expect(apply('project:Ho', complete('project:Ho', 10, vocab)).line).toBe('project:Home');
  });

  it('no options leaves the line alone', () => {
    expect(apply('zzz', complete('zzz', 3, vocab)).line).toBe('zzz');
  });

  it('replaces only the word under the caret', () => {
    const line = 'project:Home +er due:today';
    const caret = 'project:Home +er'.length;
    const r = apply(line, complete(line, caret, vocab));
    expect(r.line).toBe('project:Home +errand  due:today');
  });
});

describe('hints, ids and modifiers', () => {
  it('commands and reports carry hints', () => {
    const o = complete('mi', 2, vocab).options;
    expect(o).toEqual([{ value: 'mine', hint: 'My tasks' }]);
    expect(complete('don', 3, vocab).options[0]).toEqual({ value: 'done', hint: 'complete tasks' });
  });

  it('a number completes to task ids with their descriptions', () => {
    const o = complete('3', 1, vocab).options;
    expect(o.map((x) => x.value)).toEqual(['3', '30']);
    expect(o[0].hint).toBe('Fix the tap');
    expect(complete('30', 2, vocab).options.map((x) => x.value)).toEqual(['30']);
    expect(complete('9', 1, vocab).options).toEqual([]);
  });

  it('depends: completes the last item of a comma list', () => {
    expect(c('depends:3').options).toEqual(['depends:3', 'depends:30']);
    expect(c('depends:3,3').options).toEqual(['depends:3,3', 'depends:3,30']);
    expect(c('depends:3,-3').options).toEqual(['depends:3,3', 'depends:3,30']);
  });

  it('name.partial completes modifiers appropriate to the attribute type', () => {
    expect(c('due.be').options).toEqual(['due.before:']);
    expect(c('due.').options).toContain('due.after:');
    expect(c('due.').options).not.toContain('due.has:');
    expect(c('project.st').options).toEqual(['project.startswith:']);
    expect(c('estimate.i').options).toEqual(['estimate.is:', 'estimate.isnt:']);
    expect(c('ship.b').options).toEqual(['ship.before:', 'ship.by:']);
    expect(c('nonsense.b').options).toEqual([]);
    expect(complete('due.be', 6, vocab).options[0].hint).toBe('earlier than');
  });

  it('date values include relative shortcuts', () => {
    expect(c('due:3').options).toEqual(['due:3d']);
    expect(c('wait:1').options).toEqual(['wait:1d', 'wait:1mo', 'wait:1w']);
  });

  it('limit: has values', () => {
    expect(c('limit:p').options).toEqual(['limit:page']);
  });
});

describe('insert', () => {
  it('adds a space unless the value ends mid-token', () => {
    const cc = complete('3 do', 4, vocab);
    expect(insert('3 do', cc, 'done')).toEqual({ line: '3 done ', caret: 7 });
    expect(insert('3 do', cc, 'due:')).toEqual({ line: '3 due:', caret: 6 });
    expect(insert('3 do', cc, 'done', false)).toEqual({ line: '3 done', caret: 6 });
  });
});

describe('rc. overrides', () => {
  it('rc. offers report and context', () => {
    expect(c('rc.').options).toEqual(['rc.context:', 'rc.report.']);
    expect(c('rc.r').options).toEqual(['rc.report.']);
  });

  it('rc.report. lists reports with their descriptions', () => {
    expect(c('rc.report.').options).toEqual(['rc.report.list.', 'rc.report.mine.', 'rc.report.next.']);
    expect(c('rc.report.mi').options).toEqual(['rc.report.mine.']);
    expect(complete('rc.report.mi', 12, vocab).options[0].hint).toBe('My tasks');
  });

  it('rc.report.<name>. lists the settings', () => {
    expect(c('rc.report.next.').options).toEqual([
      'rc.report.next.columns:',
      'rc.report.next.filter:',
      'rc.report.next.labels:',
      'rc.report.next.sort:',
    ]);
    expect(c('rc.report.next.so').options).toEqual(['rc.report.next.sort:']);
  });

  it('sort values complete column names with a direction, including UDAs', () => {
    expect(c('rc.report.next.sort:du').options).toEqual(['rc.report.next.sort:due+', 'rc.report.next.sort:due-']);
    expect(c('rc.report.next.sort:due+,es').options).toEqual([
      'rc.report.next.sort:due+,estimate+',
      'rc.report.next.sort:due+,estimate-',
    ]);
    // A direction was already chosen: completing it leaves the list open for a `,`.
    const done = complete('rc.report.next.sort:due+', 24, vocab);
    expect(done.options.map((o) => o.value)).toEqual(['rc.report.next.sort:due+']);
    expect(apply('rc.report.next.sort:due+', done).line).toBe('rc.report.next.sort:due+');
  });

  it('rc.context: lists contexts', () => {
    expect(c('rc.context:w').options).toEqual(['rc.context:work']);
  });

  it('recur: offers the common periods with a plain-English hint', () => {
    expect(c('recur:we').options).toEqual(['recur:weekdays', 'recur:weekly']);
    expect(complete('recur:bi', 8, vocab).options[0].hint).toBe('every 2 weeks');
    expect(c('rec').options).toContain('recur:');
  });
});
