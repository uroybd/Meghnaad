import { describe, expect, it } from 'vitest';
import {
  deviceFile,
  deviceIdProblem,
  deviceReminders,
  hasOverrides,
  mergeDevice,
  newDeviceId,
  withDeviceHomepage,
  withDeviceKeys,
} from './deviceSettings';
import { DEFAULT_SETTINGS } from './notify';

const base = {
  version: 2,
  homepage: { report: 'next' },
  boards: [{ id: 'week' }],
  keys: { tasks: { x: 'done', y: 'start' } },
  web: { keys: { app: { 'ctrl+k': 'app.prompt' } }, other: 1 },
};

describe('a device over everyone', () => {
  it('keeps everything of everyone when the device has nothing', () => {
    expect(mergeDevice(base, null)).toBe(base);
    expect(mergeDevice(base, {})).toEqual(base);
  });

  it('lays the home page whole and the keys one by one, the rest everyone’s', () => {
    const device = {
      deviceId: 'phone',
      homepage: { board: 'week' },
      keys: { tasks: { x: null, z: 'done' } },
      tui: { keys: { app: { q: 'app.quit' } } },
      boards: [{ id: 'mine' }],
    };
    const out = mergeDevice(base, device);
    expect(out.homepage).toEqual({ board: 'week' });
    expect(out.keys).toEqual({ tasks: { x: null, y: 'start', z: 'done' } });
    expect(out.web).toEqual(base.web);
    expect(out.tui).toEqual({ keys: { app: { q: 'app.quit' } } });
    // Only the home page, the keys and the reminders are a device's own.
    expect(out.boards).toEqual(base.boards);
  });

  it('keeps as the device’s keys only what differs from everyone’s', () => {
    const draft = mergeDevice(base, {});
    const edited = { ...structuredClone(draft), keys: { tasks: { y: 'start', z: 'done' } } };
    const device = withDeviceKeys({ deviceId: 'a' }, base, edited);
    expect(device).toEqual({ deviceId: 'a', keys: { tasks: { x: null, z: 'done' } } });
    expect(mergeDevice(base, device).keys).toEqual({ tasks: { x: null, y: 'start', z: 'done' } });
    // Put back as everyone has them: no keys of its own any more.
    expect(withDeviceKeys(device, base, mergeDevice(base, {}))).toEqual({ deviceId: 'a' });
  });

  it('makes and takes away its own home page', () => {
    const d = withDeviceHomepage({ deviceId: 'a' }, null);
    expect(d).toEqual({ deviceId: 'a', homepage: null });
    expect(hasOverrides(d)).toBe(true);
    expect(withDeviceHomepage(d, undefined)).toEqual({ deviceId: 'a' });
    expect(hasOverrides({ deviceId: 'a' })).toBe(false);
  });

  it('reads its reminders over the defaults, and carries this browser’s into its first file', () => {
    expect(deviceReminders({})).toBeNull();
    expect(deviceReminders({ reminders: { enabled: true, leadMinutes: 'x' } })).toEqual({
      ...DEFAULT_SETTINGS,
      enabled: true,
    });
    const had = { ...DEFAULT_SETTINGS, enabled: true, leadMinutes: 30 };
    expect(deviceFile({ homepage: null }, 'b', 2, had)).toEqual({
      homepage: null,
      deviceId: 'b',
      version: 2,
      reminders: had,
    });
    // A file with reminders keeps its own.
    const own = { reminders: { enabled: false } };
    expect(deviceFile(own, 'b', 2, had).reminders).toEqual({ enabled: false });
  });

  it('checks and makes IDs that are file names', () => {
    expect(deviceIdProblem('laptop-2_b')).toBeNull();
    expect(deviceIdProblem('')).not.toBeNull();
    expect(deviceIdProblem('my laptop')).not.toBeNull();
    expect(deviceIdProblem('a'.repeat(65))).not.toBeNull();
    expect(newDeviceId('phone', () => 0)).toBe('phone-aaaaaa');
    expect(deviceIdProblem(newDeviceId())).toBeNull();
  });
});
