import { describe, expect, it, vi } from 'vitest';
import {
  buildCalendar,
  buildHolidata,
  buildNager,
  calendarLink,
  fetchHolidata,
  fetchNager,
  holidataPlaces,
  nagerPlaces,
  settingsOf,
  withSetting,
} from './holidays';

const US = [
  '{"locale":"en-US","region":"","date":"2026-01-01","description":"New Year\'s Day","type":"NF","notes":""}',
  '{"locale":"en-US","region":"MA","date":"2026-04-20","description":"Patriots\' Day","type":"V","notes":""}',
].join('\n');

const BD = JSON.stringify([
  { date: '2026-02-21', name: 'International Mother Language Day', global: true, counties: null, types: ['Public'] },
  { date: '2026-02-14', name: "Valentine's Day", global: true, counties: null, types: ['Observance'] },
]);

const CAL = [
  'BEGIN:VCALENDAR',
  'X-WR-CALNAME:Holidays in Bangladesh',
  'BEGIN:VEVENT',
  'DTSTART;VALUE=DATE:20260326',
  'DTEND;VALUE=DATE:20260327',
  'SUMMARY:Independence Day',
  'DESCRIPTION:Public holiday',
  'END:VEVENT',
  'BEGIN:VEVENT',
  'DTSTART;VALUE=DATE:20260214',
  'DTEND;VALUE=DATE:20260215',
  "SUMMARY:Valentine's Day",
  'DESCRIPTION:Observance',
  'END:VEVENT',
  'END:VCALENDAR',
].join('\r\n');

const answer = (by: Record<string, string | number>) =>
  vi.fn((url: string | URL | Request) => {
    const v = by[/(\d{4})(?:\.json)?$|(\d{4})\/\w+$/.exec(String(url))?.[0] ?? ''] ?? by[String(url)];
    return Promise.resolve(
      typeof v === 'string' ? new Response(v) : new Response('', { status: typeof v === 'number' ? v : 404 }),
    );
  }) as unknown as typeof fetch;

describe('Holidata', () => {
  it('lists its locales and builds national holidays, offering the regions', () => {
    expect(holidataPlaces()).toHaveLength(42);
    const plain = buildHolidata('en-US', [US], []);
    expect(plain.count).toBe(1);
    expect(plain.path).toBe('holidays/en-US.rc');
    expect(plain.regions).toEqual(['MA']);
    expect(plain.text).toContain("holiday.en-US1.name=New Year's Day\nholiday.en-US1.date=20260101\n");
    expect(plain.entries).toEqual([{ date: '20260101', end: null, name: "New Year's Day" }]);
    expect(buildHolidata('en-US', [US], ['MA']).count).toBe(2);
  });

  it('asks for each year, keeps what is there and says what is not', async () => {
    const get = answer({ '2026.json': US });
    const out = await fetchHolidata('en-US', 2026, 2027, get);
    expect(out.bodies).toEqual([US]);
    expect(out.missing).toEqual([2027]);
    expect(get).toHaveBeenCalledWith(
      'https://holidata.net/en-US/2026.json',
      expect.objectContaining({ credentials: 'omit' }),
    );
  });

  it('fails when there is nothing, when the host cannot be reached or answers badly', async () => {
    await expect(fetchHolidata('en-US', 2040, 2041, answer({}))).rejects.toThrow('no holidays for en-US');
    await expect(fetchHolidata('en-US', 2026, 2026, answer({ '2026.json': 500 }))).rejects.toThrow('answered 500');
    const offline = vi.fn(() => Promise.reject(new TypeError('Failed to fetch'))) as unknown as typeof fetch;
    await expect(fetchHolidata('en-US', 2026, 2026, offline)).rejects.toThrow("Couldn't reach holidata.net");
  });

  it('will not put anything but a locale into the address', async () => {
    const get = answer({});
    await expect(fetchHolidata('../x', 2026, 2026, get)).rejects.toThrow('not a year that can be asked for');
    expect(get).not.toHaveBeenCalled();
  });
});

describe('Nager.Date', () => {
  it('lists the countries it has, by name', async () => {
    const get = vi.fn(() =>
      Promise.resolve(
        new Response(
          JSON.stringify([
            { countryCode: 'BD', name: 'Bangladesh' },
            { countryCode: 'AL', name: 'Albania' },
          ]),
        ),
      ),
    ) as unknown as typeof fetch;
    expect((await nagerPlaces(get)).map((p) => p.id)).toEqual(['AL', 'BD']);
    expect(get).toHaveBeenCalledWith('https://date.nager.at/api/v3/AvailableCountries', expect.anything());
  });

  it('fetches a country’s years and builds only public holidays', async () => {
    const get = answer({ '2026/BD': BD });
    const out = await fetchNager('BD', 2026, 2026, get);
    expect(get).toHaveBeenCalledWith(
      'https://date.nager.at/api/v3/PublicHolidays/2026/BD',
      expect.objectContaining({ referrerPolicy: 'no-referrer' }),
    );
    const built = buildNager('BD', out.bodies, []);
    expect(built.path).toBe('holidays/nager-BD.rc');
    expect(built.entries.map((e) => e.name)).toEqual(['International Mother Language Day']);
  });
});

describe('a calendar', () => {
  it('builds the file from its events, leaving its observances out unless asked', () => {
    const built = buildCalendar('', CAL, 2026, 2026, null, null);
    expect(built.path).toBe('holidays/holidays-in-bangladesh.rc');
    expect(built.title).toBe('Holidays in Bangladesh');
    expect(built.entries.map((e) => e.name)).toEqual(['Independence Day']);
    expect(built.names.find((n) => n.name === "Valentine's Day")).toMatchObject({ kind: 'Observance', skip: true });
    const all = buildCalendar('bd', CAL, 2026, 2026, [], 'https://example.org/bd.ics');
    expect(all.entries).toHaveLength(2);
    expect(settingsOf(all.text)).toEqual({
      source: 'ics',
      key: 'bd',
      regions: [],
      skip: [],
      link: 'https://example.org/bd.ics',
    });
  });

  it('takes a webcal address as https and refuses a link that is not a public https one', () => {
    expect(calendarLink('webcal://example.org/bd.ics')).toEqual({ link: 'https://example.org/bd.ics' });
    expect(calendarLink('http://example.org/bd.ics')).toHaveProperty('problem');
    expect(calendarLink('https://127.0.0.1/x.ics')).toHaveProperty('problem');
  });
});

describe('a file we made', () => {
  it('says what it was made for; another file does not', () => {
    expect(settingsOf(buildHolidata('en-US', [US], ['MA']).text)).toMatchObject({
      source: 'holidata',
      key: 'en-US',
      regions: ['MA'],
    });
    expect(settingsOf('bulk=3\n')).toBeNull();
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
