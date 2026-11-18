// @vitest-environment jsdom
// Spec 0017: what a locked tab of the static flavour keeps to remind from, and how it reminds.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  dropList,
  dueNow,
  keepForLock,
  keptList,
  remember,
  remindWhileLocked,
  SEEN_KEY,
  SETTINGS_KEY,
  takeClicked,
} from './lockedReminders';
import type { Reminder } from './notify';

const now = Math.floor(Date.now() / 1000);
const milk: Reminder = {
  uuid: 'u1',
  status: 'pending',
  description: 'Buy milk',
  due: now - 60,
  wait: null,
  scheduled: null,
  start: null,
  active_seconds: null,
  remind_in: null,
};

const remindersOn = (on = true) => localStorage.setItem(SETTINGS_KEY, JSON.stringify({ enabled: on }));

beforeEach(() => {
  localStorage.clear();
  sessionStorage.clear();
  remember([]);
  takeClicked();
});
afterEach(() => vi.unstubAllGlobals());

describe('what a tab keeps when it locks', () => {
  it('is the list it last polled, with only what a reminder reads', () => {
    remindersOn();
    remember([{ ...milk, project: 'home', urgency: 9, annotations: ['secret plans'] } as unknown as Reminder]);
    keepForLock();
    expect(keptList()).toEqual([milk]);
    expect(sessionStorage.getItem('meghnaad-locked-reminders')).not.toContain('secret plans');
  });

  it('is nothing when reminders are off, or nothing was polled', () => {
    remember([milk]);
    keepForLock();
    expect(keptList()).toEqual([]);
    remindersOn();
    remember([]);
    keepForLock();
    expect(keptList()).toEqual([]);
  });

  it('goes once unlocked or forgotten', () => {
    remindersOn();
    remember([milk]);
    keepForLock();
    dropList();
    expect(keptList()).toEqual([]);
  });
});

describe('reminding while locked', () => {
  it('says each reminder once, and records it as the notifier does', () => {
    remindersOn();
    const first = dueNow([milk], now);
    expect(first.map((n) => n.title)).toEqual(['Due now']);
    expect(Object.keys(JSON.parse(localStorage.getItem(SEEN_KEY) ?? '{}'))).toEqual([first[0].key]);
    expect(dueNow([milk], now + 15)).toEqual([]);
  });

  it('says nothing when reminders are off', () => {
    remindersOn(false);
    expect(dueNow([milk], now)).toEqual([]);
  });

  it('shows it on the page without permission for notifications', () => {
    remindersOn();
    vi.stubGlobal('Notification', { permission: 'denied' });
    const inPage = vi.fn();
    const stop = remindWhileLocked([milk], inPage);
    stop();
    expect(inPage).toHaveBeenCalledWith(expect.objectContaining({ title: 'Due now', body: 'Buy milk' }));
  });

  it('as a notification, whose click remembers the task to open once unlocked', () => {
    remindersOn();
    const shown: { title: string; onclick?: () => void; close: () => void }[] = [];
    class FakeNotification {
      static permission = 'granted';
      onclick?: () => void;
      close = vi.fn();
      constructor(public title: string) {
        shown.push(this);
      }
    }
    vi.stubGlobal('Notification', FakeNotification);
    const inPage = vi.fn();
    const stop = remindWhileLocked([milk], inPage);
    stop();
    expect(inPage).not.toHaveBeenCalled();
    expect(shown.map((n) => n.title)).toEqual(['Due now']);
    expect(takeClicked()).toBeNull();
    shown[0].onclick?.();
    expect(takeClicked()).toBe('u1');
    expect(takeClicked()).toBeNull();
  });

  it('keeps checking every 15 seconds until stopped', () => {
    vi.useFakeTimers();
    try {
      // No heads-up before the due date: only "Due now", once it comes.
      localStorage.setItem(SETTINGS_KEY, JSON.stringify({ enabled: true, leadMinutes: 0 }));
      vi.stubGlobal('Notification', { permission: 'denied' });
      const later = { ...milk, uuid: 'u2', due: Math.floor(Date.now() / 1000) + 10 };
      const inPage = vi.fn();
      const stop = remindWhileLocked([later], inPage);
      expect(inPage).not.toHaveBeenCalled();
      vi.advanceTimersByTime(15_000);
      expect(inPage).toHaveBeenCalledTimes(1);
      stop();
      vi.advanceTimersByTime(60_000);
      expect(inPage).toHaveBeenCalledTimes(1);
    } finally {
      vi.useRealTimers();
    }
  });
});
