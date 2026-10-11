// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { bundledIcons, CDN, fetchCatalog, fetchIcon, LUCIDE_VERSION, svgToShapes } from './lucide';
import { lucide } from './lucideLib.svelte';

// The file Lucide serves for `circle-x`.
const CIRCLE_X = `<!-- @license lucide-static v1.52.0 - ISC -->
<svg class="lucide lucide-circle-x" xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"
  fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <circle cx="12" cy="12" r="10" />
  <path d="m15 9-6 6" />
  <path d="m9 9 6 6" />
</svg>`;

const svg = (inner: string) => `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">${inner}</svg>`;
const reply =
  (body: string, status = 200) =>
  () =>
    Promise.resolve(new Response(body, { status }));

afterEach(() => {
  vi.unstubAllGlobals();
  localStorage.clear();
});

describe('svgToShapes', () => {
  it('reads the shapes of a Lucide file', () => {
    expect(svgToShapes(CIRCLE_X)).toEqual([
      ['circle', { cx: '12', cy: '12', r: '10' }],
      ['path', { d: 'm15 9-6 6' }],
      ['path', { d: 'm9 9 6 6' }],
    ]);
  });

  it('reads every kind of shape Lucide uses', () => {
    const s = svgToShapes(
      svg(
        '<rect x="3" y="3" width="18" height="18" rx="2"/><line x1="1" y1="2" x2="3" y2="4"/><polyline points="1 2, 3 4"/><polygon points="1 2 3 4 5 6"/><ellipse cx="1" cy="2" rx="3" ry="4"/>',
      ),
    );
    expect(s?.map(([tag]) => tag)).toEqual(['rect', 'line', 'polyline', 'polygon', 'ellipse']);
  });

  it('refuses a file with anything else in it', () => {
    for (const inner of [
      '<path d="M0 0"/><script>alert(1)</script>',
      '<foreignObject><div>x</div></foreignObject>',
      '<image href="https://example.com/x.png"/>',
      '<a href="javascript:alert(1)"><path d="M0 0"/></a>',
      '<style>svg { display: none }</style>',
      '<g><path d="M0 0"/></g>',
      '<use href="#x"/>',
      '',
    ])
      expect(svgToShapes(svg(inner)), inner).toBeNull();
  });

  it('refuses what is not an SVG or is not well formed', () => {
    for (const text of ['', 'not xml', '<html><body></body></html>', '<svg><path d="M0 0"></svg>', '{"a":1}'])
      expect(svgToShapes(text), text).toBeNull();
  });

  it('drops an attribute that is not a plain number or path, and any it does not list', () => {
    const [[, attrs]] = svgToShapes(
      svg(
        '<path d="M0 0 L1 1" onclick="alert(1)" style="fill:red" fill="url(#x)" stroke="red" class="a" opacity="0.5"/>',
      ),
    )!;
    expect(attrs).toEqual({ d: 'M0 0 L1 1' });
    const [[, bad]] = svgToShapes(svg('<circle cx="javascript:1" cy="12" r="calc(1px)" />'))!;
    expect(bad).toEqual({ cy: '12' });
    const [[, path]] = svgToShapes(svg('<path d="M0 0 url(javascript:alert(1))"/>'))!;
    expect(path).toEqual({});
  });
});

describe('bundledIcons', () => {
  it('has the icons the app carries, by their Lucide names', () => {
    for (const name of ['check', 'trash-2', 'rows-3', 'arrow-right-to-line', 'sliders-horizontal', 'ellipsis'])
      expect(bundledIcons.has(name), name).toBe(true);
    expect(bundledIcons.has('circle-x')).toBe(false);
  });
});

describe('fetchIcon', () => {
  it('asks the CDN for the file at the pinned version, and gives its shapes', async () => {
    const get = vi.fn(reply(CIRCLE_X));
    const r = await fetchIcon('circle-x', get as never);
    expect(get).toHaveBeenCalledWith(`https://cdn.jsdelivr.net/npm/lucide-static@${LUCIDE_VERSION}/icons/circle-x.svg`);
    expect(CDN).toContain(`lucide-static@${LUCIDE_VERSION}`);
    expect(r).toMatchObject({
      shapes: [
        ['circle', { r: '10' }],
        ['path', {}],
        ['path', {}],
      ],
    });
  });

  it('tells an icon that does not exist from one that could not be had', async () => {
    expect(await fetchIcon('no-such', reply('', 404) as never)).toEqual({ missing: true });
    expect(await fetchIcon('x', reply('', 500) as never)).toEqual({ failed: true });
    expect(await fetchIcon('x', (() => Promise.reject(new Error('offline'))) as never)).toEqual({ failed: true });
    expect(await fetchIcon('x', reply('<svg><script/></svg>') as never)).toEqual({ failed: true });
  });

  it('never asks for a name that could point somewhere else', async () => {
    const get = vi.fn(reply(CIRCLE_X));
    for (const name of ['../x', 'a/b', 'A', 'a b', '', 'x?y=1', '..%2f'])
      expect(await fetchIcon(name, get as never)).toEqual({ missing: true });
    expect(get).not.toHaveBeenCalled();
  });
});

describe('fetchCatalog', () => {
  it('reads the names and what they are found by', async () => {
    const c = await fetchCatalog(
      reply(JSON.stringify({ x: ['close', 'cancel'], 'circle-x': ['cancel'], 'Bad/Name': [] })) as never,
    );
    expect(c).toEqual({ x: ['close', 'cancel'], 'circle-x': ['cancel'] });
  });

  it('gives nothing when it could not be had', async () => {
    expect(await fetchCatalog(reply('', 500) as never)).toBeNull();
    expect(await fetchCatalog(reply('[]') as never)).toBeNull();
    expect(await fetchCatalog(reply('nope') as never)).toBeNull();
  });
});

describe('what is known about icons', () => {
  it('fetches an icon once, and keeps in the browser only the ones that commands use', async () => {
    const get = vi.fn(reply(CIRCLE_X));
    vi.stubGlobal('fetch', get);
    lucide.keep(['circle-x']);
    lucide.request('circle-x');
    lucide.request('circle-x');
    lucide.request('just-looked');
    await vi.waitFor(() => expect(lucide.shapes['circle-x'] && lucide.shapes['just-looked']).toBeTruthy());
    expect(get).toHaveBeenCalledTimes(2);
    expect(Object.keys(JSON.parse(localStorage.getItem('tw-web-icons')!))).toEqual(['circle-x']);
    // Nothing is kept for a command that was taken away.
    lucide.keep([]);
    expect(JSON.parse(localStorage.getItem('tw-web-icons')!)).toEqual({});
  });

  it('remembers an icon that does not exist, and tries again for one that could not be had', async () => {
    const get = vi.fn(reply('', 404));
    vi.stubGlobal('fetch', get);
    lucide.request('not-an-icon');
    await vi.waitFor(() => expect(lucide.missing['not-an-icon']).toBe(true));
    lucide.request('not-an-icon');
    expect(get).toHaveBeenCalledTimes(1);

    const down = vi.fn(() => Promise.reject(new Error('offline')));
    vi.stubGlobal('fetch', down);
    lucide.request('later');
    await vi.waitFor(() => expect(down).toHaveBeenCalledTimes(1));
    await Promise.resolve();
    lucide.request('later');
    await vi.waitFor(() => expect(down).toHaveBeenCalledTimes(2));
    expect(lucide.missing['later']).toBeUndefined();
  });

  it('searches the names and the words they are found by, names first', async () => {
    vi.stubGlobal(
      'fetch',
      reply(
        JSON.stringify({
          x: ['close', 'cancel'],
          'circle-x': ['cancel', 'close'],
          'x-circle-ish': [],
          check: ['done'],
        }),
      ),
    );
    await lucide.loadCatalog();
    expect(lucide.search('')).toEqual(['check', 'circle-x', 'x', 'x-circle-ish']);
    expect(lucide.search('circle x')).toEqual(['circle-x', 'x-circle-ish']);
    expect(lucide.search('close')).toEqual(['circle-x', 'x']);
    expect(lucide.search('x')).toEqual(['circle-x', 'x', 'x-circle-ish']);
    expect(lucide.search('done')).toEqual(['check']);
    expect(lucide.search('zzz')).toEqual([]);
  });
});
