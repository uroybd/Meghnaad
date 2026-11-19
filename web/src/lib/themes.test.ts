import { describe, expect, it } from 'vitest';
import {
  THEMES,
  chooseTheme,
  chosenTheme,
  colourOff,
  expandIncludes,
  includedTheme,
  themeLines,
  withColour,
} from './themes';

const dark = `# header
color.active=rgb555 on rgb410   # active
color=1   # Enable color
rule.precedence.color=deleted,completed,active

color.tag.next=rgb440
`;

describe('themeLines', () => {
  it('keeps the setting lines and drops comments, whole-line or trailing', () => {
    expect(themeLines(dark)).toEqual([
      'color.active=rgb555 on rgb410',
      'color=1',
      'rule.precedence.color=deleted,completed,active',
      'color.tag.next=rgb440',
    ]);
  });
});

describe('chooseTheme', () => {
  const rc = 'bulk=5\ncolor.overdue=red\ncolor=off\nrule.color.merge=off\nuda.size.type=string\n';
  it('puts the theme in a file of its own, as it is, and includes it in place of the colour lines', () => {
    const { main, theme } = chooseTheme(rc, 'dark-256', dark);
    expect(theme).toEqual({ path: 'themes/dark-256.rc', text: dark });
    expect(main).toBe('bulk=5\ncolor=off\nuda.size.type=string\n\ninclude themes/dark-256.rc\n');
    expect(chosenTheme(main)).toBe('dark-256');
  });

  it("choosing Taskwarrior's defaults just clears the colour lines, with no file", () => {
    expect(chooseTheme(rc, 'default', '')).toEqual({ main: 'bulk=5\ncolor=off\nuda.size.type=string\n', theme: null });
  });

  it('a second theme takes the place of the first, and a theme pasted in as lines goes too', () => {
    const once = chooseTheme('bulk=5\n# theme: meghnaad\ncolor.due=red\n', 'dark-256', dark).main;
    const twice = chooseTheme(once, 'light-16', 'color.active=black on yellow').main;
    expect(twice).toBe('bulk=5\n\ninclude themes/light-16.rc\n');
    expect(chooseTheme(twice, 'default', '').main).toBe('bulk=5\n');
    expect(chosenTheme('bulk=5\n')).toBeNull();
  });
});

describe('colour on and off', () => {
  it('writes and removes the colour=off line, leaving the rest', () => {
    expect(withColour('bulk=5\n', false)).toBe('bulk=5\ncolor=off\n');
    expect(withColour('bulk=5\ncolor=off\n', true)).toBe('bulk=5\n');
    expect(withColour('', false)).toBe('color=off\n');
    expect(withColour('color=off\n', false)).toBe('color=off\n');
    // color.* lines are not the switch.
    expect(withColour('color.active=red\n', true)).toBe('color.active=red\n');
  });
  it('reads whether it is off', () => {
    expect(colourOff('color=off\n')).toBe(true);
    expect(colourOff('color = 0 # no\n')).toBe(true);
    expect(colourOff('color=on\n')).toBe(false);
    expect(colourOff('color.active=red\n')).toBe(false);
  });
});

describe('include', () => {
  it('recognises the bundled themes by name, with or without a path', () => {
    expect(includedTheme('include dark-256.theme')).toBe('dark-256');
    expect(includedTheme('  include /usr/share/doc/task/rc/solarized-dark-256.theme ')).toBe('solarized-dark-256');
    expect(includedTheme('include ~/mine.theme')).toBeNull();
    expect(includedTheme('include something.rc')).toBeNull();
    expect(includedTheme('color.active=red')).toBeNull();
  });
  it('expands only those, in place', async () => {
    const out = await expandIncludes(
      'bulk=5\ninclude dark-256.theme\ninclude ~/mine.theme\nlimit=9',
      async (id) => `color.active=${id}`,
    );
    expect(out).toBe('bulk=5\n# theme: dark-256\ncolor.active=dark-256\ninclude ~/mine.theme\nlimit=9');
  });
});

describe('the list', () => {
  it("has Taskwarrior's defaults first and each theme once", () => {
    expect(THEMES[0].id).toBe('default');
    expect(new Set(THEMES.map((t) => t.id)).size).toBe(THEMES.length);
    expect(THEMES.length).toBe(17);
  });
});
