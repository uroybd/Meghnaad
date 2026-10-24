import { describe, expect, it, vi } from 'vitest';
import { build, fetchYears, locales, pathFor, settingsOf, withSetting } from './holidays';

const US = [
  '{"locale":"en-US","region":"","date":"2026-01-01","description":"New Year\'s Day","type":"NF","notes":""}',
  '{"locale":"en-US","region":"MA","date":"2026-04-20","description":"Patriots\' Day","type":"V","notes":""}',
].join('\n');

const answer = (by: Record<number, string | number>) =>
  vi.fn((url: string | URL | Request) => {
    const year = Number(/(\d{4})\.json$/.exec(String(url))?.[1]);
    const v = by[year];
    return Promise.resolve(
      typeof v === 'string' ? new Response(v) : new Response('', { status: typeof v === 'number' ? v : 404 }),
    );
  }) as unknown as typeof fetch;

describe('the locales and the file made from them', () => {
  it('lists Holidata locales with a name to look for', () => {
    const all = locales();
    expect(all).toHaveLength(42);
    expect(all.find((l) => l.locale === 'en-US')?.label).toBe('United States (English)');
    expect(pathFor('en-US')).toBe('holidays/en-US.rc');
  });

  it('builds national holidays, offers the regions, and remembers what it was made for', () => {
    const plain = build('en-US', [US], []);
    expect(plain.count).toBe(1);
    expect(plain.regions).toEqual(['MA']);
    expect(plain.text).toContain("holiday.en-US1.name=New Year's Day\nholiday.en-US1.date=20260101\n");
    const withMa = build('en-US', [US], ['MA']);
    expect(withMa.count).toBe(2);
    expect(settingsOf(withMa.text)).toEqual({ locale: 'en-US', regions: ['MA'] });
    expect(settingsOf('bulk=3\n')).toBeNull();
  });
});

describe('fetching the years', () => {
  it('asks for each year, keeps what is there and says what is not', async () => {
    const get = answer({ 2026: US, 2027: 404 });
    const out = await fetchYears('en-US', 2026, 2027, get);
    expect(out.bodies).toEqual([US]);
    expect(out.missing).toEqual([2027]);
    expect(get).toHaveBeenCalledWith(
      'https://holidata.net/en-US/2026.json',
      expect.objectContaining({ credentials: 'omit' }),
    );
  });

  it('fails when there is nothing, or the host cannot be reached or answers badly', async () => {
    await expect(fetchYears('en-US', 2040, 2041, answer({}))).rejects.toThrow('no holidays for en-US');
    await expect(fetchYears('en-US', 2026, 2026, answer({ 2026: 500 }))).rejects.toThrow('answered 500');
    const offline = vi.fn(() => Promise.reject(new TypeError('Failed to fetch'))) as unknown as typeof fetch;
    await expect(fetchYears('en-US', 2026, 2026, offline)).rejects.toThrow("Couldn't reach holidata.net");
  });

  it('will not put anything but a locale into the address', async () => {
    const get = answer({});
    await expect(fetchYears('../x', 2026, 2026, get)).rejects.toThrow('not a year Holidata can have');
    expect(get).not.toHaveBeenCalled();
  });
});

describe('withSetting', () => {
  it('adds a line, or changes the one that is there', () => {
    expect(withSetting('bulk=3\n', 'calendar.holidays', 'sparse')).toBe('bulk=3\ncalendar.holidays=sparse\n');
    expect(withSetting('', 'a', '1')).toBe('a=1\n');
    expect(withSetting('calendar.holidays=none\nbulk=3\n', 'calendar.holidays', 'sparse')).toBe(
      'calendar.holidays=sparse\nbulk=3\n',
    );
  });
});
