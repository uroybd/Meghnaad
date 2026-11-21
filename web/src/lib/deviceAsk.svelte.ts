// The drawer that names this device (spec 0027): asked for on the first save of its own settings, and offered on the
// first switch to This device when other devices already have some. `DeviceDrawer.svelte` draws it.

import { newDeviceId } from './deviceSettings';
import { store } from './store.svelte';

type Ask = { mode: 'name'; id: string; resolve: (id: string | null) => void } | { mode: 'choose'; ids: string[] };

export const deviceAsk = $state<{ now: Ask | null }>({ now: null });

/** A word for this kind of device, to start its ID with. */
function kind(): string {
  try {
    return /Android|iPhone|iPad|Mobile/i.test(navigator.userAgent) ? 'phone' : 'browser';
  } catch {
    return 'device';
  }
}

/** This device's ID, asking for one (prefilled with a new one) when it has none yet; `null` when the person declined. */
export function ensureDeviceId(): Promise<string | null> {
  if (store.deviceId) return Promise.resolve(store.deviceId);
  return new Promise((resolve) => {
    deviceAsk.now = { mode: 'name', id: newDeviceId(kind()), resolve };
  });
}

/** The first switch to This device: when other devices have settings of their own, offer to use one of them. */
export async function offerDevices(): Promise<void> {
  if (store.deviceId) return;
  try {
    const ids = await store.listDevices();
    if (ids.length) deviceAsk.now = { mode: 'choose', ids };
  } catch {
    // Not offered: a new one is made on the first save, as with none in the store.
  }
}
