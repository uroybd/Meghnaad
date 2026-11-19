const NARROW = '(max-width: 760px)';
const FOLDED_KEY = 'meghnaad.rc.folded';
const CLOSED_KEY = 'meghnaad.rc.closed';

function remembered<T>(key: string, fallback: T): T {
  try {
    const v = localStorage.getItem(key);
    return v === null ? fallback : (JSON.parse(v) as T);
  } catch {
    return fallback;
  }
}

function remember(key: string, value: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    /* private window: not remembered, and that is fine */
  }
}

/**
 * How the taskrc tab's workspace is laid out. The list of files is folded away or not, and its folders open or closed,
 * remembered in this browser; on a narrow screen it starts folded, and opens over the editor. The workspace fills what is
 * left of the page's scrolling area below it, so the editor takes the height and Apply stays on screen, whatever sits
 * above it (the tabs, a warning) and below it (the console bar). Called while the panel starts, which its effect belongs to.
 */
export function rcLayout(workspace: () => HTMLElement | undefined) {
  const narrow = typeof window !== 'undefined' && !!window.matchMedia?.(NARROW).matches;
  let folded = $state(narrow || remembered(FOLDED_KEY, false));
  let closed = $state(new Set<string>(remembered<string[]>(CLOSED_KEY, [])));
  let fill = $state<number | null>(null);

  $effect(() => {
    const ws = workspace();
    const area = ws?.closest('main') as HTMLElement | null;
    if (!ws || !area || typeof ResizeObserver === 'undefined') return;
    const measure = () => {
      const top = ws.getBoundingClientRect().top - area.getBoundingClientRect().top + area.scrollTop;
      const pad = parseFloat(getComputedStyle(area).paddingBottom) || 0;
      fill = Math.max(360, Math.floor(area.clientHeight - top - pad - 4));
    };
    measure();
    const seen = new ResizeObserver(measure);
    seen.observe(area);
    return () => seen.disconnect();
  });

  return {
    narrow,
    get folded() {
      return folded;
    },
    /** The folders closed in the list. */
    get closed() {
      return closed;
    },
    /** The workspace's height in pixels, once measured. */
    get fill() {
      return fill;
    },
    /** Fold the list away or bring it back (remembered, except on a narrow screen, where it always starts folded). */
    fold(next: boolean) {
      folded = next;
      if (!narrow) remember(FOLDED_KEY, next);
    },
    /** Open a closed folder, or close an open one (remembered). */
    toggle(f: string) {
      const next = new Set(closed);
      if (!next.delete(f)) next.add(f);
      closed = next;
      remember(CLOSED_KEY, [...next]);
    },
    /** Show a folder open, as a new one is (not remembered: it is open by default). */
    reveal(f: string) {
      closed = new Set([...closed].filter((c) => c !== f));
    },
  };
}
