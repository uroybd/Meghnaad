// Holidays for the taskrc folder, from Holidata, Nager.Date or a calendar link. What each source's answers mean, the file made
// from them and which countries there are, are the engine's (`tc_core::holidata`, `nager`, `ics`, through the helpers); the page
// only fetches (Holidata and Nager.Date from the page, which `web/public/_headers` allows; a calendar through the Worker) and
// puts the result in the folder.

import { callHelper } from '../engine/helpers';

export type Source = 'holidata' | 'nager' | 'ics';

export interface Place {
  /** What is chosen: a Holidata locale (`en-US`) or a Nager.Date country code (`BD`). */
  id: string;
  label: string;
}
export interface Entry {
  date: string;
  /** The last day, when it lasts longer than one. */
  end: string | null;
  name: string;
}
export interface CalendarName {
  name: string;
  count: number;
  /** What the calendar calls it (Google's "Public holiday", "Observance"). */
  kind: string;
  /** Left out unless asked for. */
  skip: boolean;
}
/** What the engine made from a source's answers. */
export interface Built {
  path: string;
  key: string;
  text: string;
  count: number;
  regions: string[];
  skipped: number;
  entries: Entry[];
  names: CalendarName[];
  title: string;
}
/** What Add holidays was asked to do. */
export interface Choice {
  path: string;
  text: string;
  /** Put `include <path>` into the main taskrc. */
  include: boolean;
  /** Set `calendar.holidays` in the main taskrc so the calendar shows them. */
  calendar: boolean;
}
/** What a file made by Add holidays says it was made for. */
export interface Made {
  source: Source;
  key: string;
  regions: string[];
  skip: string[];
  link: string | null;
}

/** A source's file is at most this long (a locale has a few dozen holidays a year); more is not what it should be. */
const MAX_BODY = 200_000;

export const holidataPlaces = (): Place[] =>
  callHelper<{ locale: string; label: string }[]>('holidata_locales').map((l) => ({ id: l.locale, label: l.label }));

const parse = (body: string) => callHelper<{ code: string; name: string }[]>('nager_countries', body);

export const settingsOf = (text: string): Made | null => callHelper<Made | null>('holidays_settings', text);

export interface Fetched {
  bodies: string[];
  /** Years the source has no file for. */
  missing: number[];
}

type Get = typeof fetch;
const defaultGet: Get = (...a) => fetch(...a);

async function text(url: string, get: Get): Promise<{ status: number; body: string }> {
  let res: Response;
  try {
    res = await get(url, { credentials: 'omit', referrerPolicy: 'no-referrer' });
  } catch {
    throw new Error(`Couldn't reach ${new URL(url).host}. Check the connection and try again.`);
  }
  const body = res.status === 404 ? '' : await res.text();
  if (body.length > MAX_BODY) throw new Error(`${new URL(url).host} sent more than a holiday list should be.`);
  return { status: res.status, body };
}

/** The files for `first` to `last`. A year with no file is reported; no file at all, or no answer, throws. */
async function years(
  what: string,
  urlOf: (year: number) => string | null,
  first: number,
  last: number,
  get: Get,
): Promise<Fetched> {
  const out: Fetched = { bodies: [], missing: [] };
  for (let year = first; year <= last; year++) {
    const url = urlOf(year);
    if (!url) throw new Error(`${what} ${year} is not a year that can be asked for.`);
    const { status, body } = await text(url, get);
    if (status === 404) out.missing.push(year);
    else if (status < 200 || status > 299) throw new Error(`${new URL(url).host} answered ${status} for ${year}.`);
    else out.bodies.push(body);
  }
  if (out.bodies.length === 0) throw new Error(`There are no holidays for ${what} in those years.`);
  return out;
}

export const fetchHolidata = (locale: string, first: number, last: number, get: Get = defaultGet) =>
  years(locale, (y) => callHelper<string | null>('holidata_url', locale, y), first, last, get);

export const fetchNager = (country: string, first: number, last: number, get: Get = defaultGet) =>
  years(country, (y) => callHelper<string | null>('nager_url', country, y), first, last, get);

/** The countries Nager.Date has holidays for. */
export async function nagerPlaces(get: Get = defaultGet): Promise<Place[]> {
  const { status, body } = await text(callHelper<string>('nager_countries_url'), get);
  if (status < 200 || status > 299) throw new Error(`date.nager.at answered ${status}.`);
  return parse(body).map((c) => ({ id: c.code, label: c.name }));
}

export const buildHolidata = (locale: string, bodies: string[], regions: string[]) =>
  callHelper<Built>('holidata_rc', locale, bodies, regions);

export const buildNager = (country: string, bodies: string[], regions: string[]) =>
  callHelper<Built>('nager_rc', country, bodies, regions);

/** The file for a calendar's events in `first` to `last`; `skip` is the names to leave out (`null`: its observances), `id`
 *  names the file (empty: the calendar's own name). */
export const buildCalendar = (
  id: string,
  text: string,
  first: number,
  last: number,
  skip: string[] | null,
  link: string | null,
) => callHelper<Built>('ics_rc', id, text, first, last, skip, link);

/** A link for a calendar as the Worker would take it (`webcal://` is `https://`), or what is wrong with it. */
export function calendarLink(link: string): { link: string } | { problem: string } {
  return callHelper<{ link: string } | { problem: string }>(
    'holiday_link',
    link.trim().replace(/^webcal:\/\//i, 'https://'),
  );
}

/** `name=value` in a taskrc's text: that line's value replaced if it is there, the line added if not. */
export function withSetting(text: string, name: string, value: string): string {
  const lines = text.split('\n');
  const at = lines.findIndex((l) => l.startsWith(`${name}=`));
  if (at >= 0) {
    lines[at] = `${name}=${value}`;
    return lines.join('\n');
  }
  return `${text}${text === '' || text.endsWith('\n') ? '' : '\n'}${name}=${value}\n`;
}
