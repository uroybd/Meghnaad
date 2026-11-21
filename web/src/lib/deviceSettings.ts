// A device's own settings (spec 0027): a partial `settings.json` kept as `devices/<id>.json`, laid over everyone's. What
// a device may have of its own is the home page, the keys (binding by binding) and the reminders; the rest is everyone's.
// Ported to Kotlin with these tests (`DeviceSettings.kt`).

import { DEFAULT_SETTINGS, type NotifySettings } from './notify';

type Raw = Record<string, unknown>;

/** Where this browser remembers which device's settings it uses. */
export const DEVICE_ID_KEY = 'meghnaad-device-id';

/** The sections of `settings.json` that hold keys: `keys` for every front end, the others for one alone. */
const KEY_SECTIONS = ['keys', 'web', 'mobile', 'tui'] as const;

const isObject = (v: unknown): v is Raw => !!v && typeof v === 'object' && !Array.isArray(v);

/** Why `id` can't name a device's settings, or `null`: letters, digits, `-` and `_`, 1 to 64 (`webapi::device_id_problem`). */
export function deviceIdProblem(id: string): string | null {
  if (id.length < 1 || id.length > 64) return 'A device ID is 1 to 64 characters.';
  if (!/^[A-Za-z0-9_-]+$/.test(id)) return 'A device ID is letters, digits, - and _.';
  return null;
}

/** A new device's ID, to keep or type over: a word for the kind of device and a few random letters. */
export function newDeviceId(kind = 'device', random: () => number = Math.random): string {
  const letters = 'abcdefghijkmnpqrstuvwxyz23456789';
  let tail = '';
  for (let i = 0; i < 6; i++) tail += letters[Math.floor(random() * letters.length)];
  return `${kind}-${tail}`;
}

/** The groups of keys a section holds (`keys` itself, or `web.keys`), as `{ group: { key: command | null } }`. */
function groupsOf(raw: Raw, section: (typeof KEY_SECTIONS)[number]): Record<string, Record<string, unknown>> {
  const at = section === 'keys' ? raw.keys : isObject(raw[section]) ? (raw[section] as Raw).keys : undefined;
  if (!isObject(at)) return {};
  const out: Record<string, Record<string, unknown>> = {};
  for (const [g, keys] of Object.entries(at)) if (isObject(keys)) out[g] = keys;
  return out;
}

function setGroups(raw: Raw, section: (typeof KEY_SECTIONS)[number], groups: Record<string, Record<string, unknown>>) {
  const empty = Object.keys(groups).length === 0;
  if (section === 'keys') {
    if (empty) delete raw.keys;
    else raw.keys = groups;
    return;
  }
  const holder = isObject(raw[section]) ? { ...(raw[section] as Raw) } : {};
  if (empty) delete holder.keys;
  else holder.keys = groups;
  if (Object.keys(holder).length) raw[section] = holder;
  else delete raw[section];
}

/**
 * What this device uses: `base` (`settings.json`) with `device` laid over it. The home page whole, the keys one by one
 * (a key the device writes, `null` included, is its own), the reminders the device's alone; the rest is `base`'s.
 */
export function mergeDevice(base: Raw, device: Raw | null): Raw {
  if (!device) return base;
  const out: Raw = { ...base };
  if ('homepage' in device) out.homepage = device.homepage;
  for (const section of KEY_SECTIONS) {
    const mine = groupsOf(device, section);
    if (!Object.keys(mine).length) continue;
    const groups = { ...groupsOf(base, section) };
    for (const [g, keys] of Object.entries(mine)) groups[g] = { ...(groups[g] ?? {}), ...keys };
    setGroups(out, section, groups);
  }
  return out;
}

/**
 * `device` with its keys made what tells `draft` (the keys as edited over the merged settings) from `base`: each key
 * whose command differs, and `null` for one `base` binds that the draft took out. A key the same as `base`'s is left
 * out, so putting a key back as it was removes the override.
 */
export function withDeviceKeys(device: Raw, base: Raw, draft: Raw): Raw {
  const out: Raw = { ...device };
  for (const section of KEY_SECTIONS) {
    const was = groupsOf(base, section);
    const now = groupsOf(draft, section);
    const diff: Record<string, Record<string, unknown>> = {};
    for (const g of new Set([...Object.keys(was), ...Object.keys(now)])) {
      const a = was[g] ?? {};
      const b = now[g] ?? {};
      const keys: Record<string, unknown> = {};
      for (const k of new Set([...Object.keys(a), ...Object.keys(b)])) {
        const v = k in b ? b[k] : null;
        if (!(k in a) || JSON.stringify(a[k]) !== JSON.stringify(v)) {
          // Missing on both sides counts as the same.
          if (!(k in a) && !(k in b)) continue;
          keys[k] = v;
        }
      }
      if (Object.keys(keys).length) diff[g] = keys;
    }
    setGroups(out, section, diff);
  }
  return out;
}

/** `device` with the home page its own (`homepage` given) or everyone's again (`undefined`). */
export function withDeviceHomepage(device: Raw, homepage: unknown): Raw {
  const out: Raw = { ...device };
  if (homepage === undefined) delete out.homepage;
  else out.homepage = homepage;
  return out;
}

/** The reminders a device's file holds, over the defaults; `null` when it holds none. */
export function deviceReminders(device: Raw | null): NotifySettings | null {
  if (!device || !isObject(device.reminders)) return null;
  const r = device.reminders;
  const out: NotifySettings = { ...DEFAULT_SETTINGS };
  if (typeof r.enabled === 'boolean') out.enabled = r.enabled;
  if (typeof r.leadMinutes === 'number') out.leadMinutes = r.leadMinutes;
  if (typeof r.allDayHour === 'number') out.allDayHour = r.allDayHour;
  if (typeof r.waitOver === 'boolean') out.waitOver = r.waitOver;
  return out;
}

/** A device's file with its ID and version set (that of everyone's), and the reminders this browser had before when it
 * holds none of its own (the first save). */
export function deviceFile(device: Raw, id: string, version: unknown, reminders: NotifySettings | null): Raw {
  const out: Raw = { ...device, deviceId: id };
  if (version !== undefined) out.version = version;
  if (!isObject(out.reminders) && reminders) out.reminders = { ...reminders };
  return out;
}

/** Whether a device's file holds an override of anything: a home page, a key or reminders. */
export function hasOverrides(device: Raw | null): boolean {
  if (!device) return false;
  if ('homepage' in device || isObject(device.reminders)) return true;
  return KEY_SECTIONS.some((s) => Object.keys(groupsOf(device, s)).length > 0);
}
