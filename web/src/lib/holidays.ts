// Holidays from Holidata, for the taskrc folder. Which locales there are, the years' files and the rc text made from them
// are the engine's (`tc_core::holidata`, through the helpers); the page only fetches the files (the one place it talks to
// holidata.net, which `web/public/_headers` allows) and puts the result in the folder.

import { callHelper } from '../engine/helpers';

export interface Locale {
  locale: string;
  label: string;
}
/** What the engine made from the years' files. */
export interface Built {
  text: string;
  count: number;
  regions: string[];
  skipped: number;
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

/** A year's file is at most this long (a locale has a few dozen holidays a year); more is not Holidata. */
const MAX_BODY = 200_000;

export const locales = (): Locale[] => callHelper<Locale[]>('holidata_locales');

/** Where holidays for a locale are kept in the folder. */
export const pathFor = (locale: string): string => `holidays/${locale}.rc`;

export const build = (locale: string, bodies: string[], regions: string[]): Built =>
  callHelper<Built>('holidata_rc', locale, bodies, regions);

/** The locale and regions a file made by Add holidays was made for, or `null` when it is not one. */
export const settingsOf = (text: string): { locale: string; regions: string[] } | null => {
  const found = callHelper<[string, string[]] | null>('holidata_settings', text);
  return found ? { locale: found[0], regions: found[1] } : null;
};

export interface Fetched {
  bodies: string[];
  /** Years Holidata has no file for. */
  missing: number[];
}

/** The files for `first` to `last` of a locale. A year with no file is reported; no file at all, or no answer, throws. */
export async function fetchYears(
  locale: string,
  first: number,
  last: number,
  get: typeof fetch = (...a) => fetch(...a),
): Promise<Fetched> {
  const out: Fetched = { bodies: [], missing: [] };
  for (let year = first; year <= last; year++) {
    const url = callHelper<string | null>('holidata_url', locale, year);
    if (!url) throw new Error(`${locale} ${year} is not a year Holidata can have.`);
    let res: Response;
    try {
      res = await get(url, { credentials: 'omit', referrerPolicy: 'no-referrer' });
    } catch {
      throw new Error("Couldn't reach holidata.net. Check the connection and try again.");
    }
    if (res.status === 404) {
      out.missing.push(year);
      continue;
    }
    if (!res.ok) throw new Error(`holidata.net answered ${res.status} for ${year}.`);
    const body = await res.text();
    if (body.length > MAX_BODY) throw new Error(`The file for ${year} is larger than a holiday list should be.`);
    out.bodies.push(body);
  }
  if (out.bodies.length === 0) throw new Error(`Holidata has no holidays for ${locale} in those years.`);
  return out;
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
