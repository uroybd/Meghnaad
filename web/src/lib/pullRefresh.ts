// Pull down at the top of a list to refresh it, as phones do: the page follows the finger with some resistance, and
// letting go far enough runs `onrefresh` (a sync, then a redraw) while the indicator spins.

/** How far (in pixels, after the resistance) the page has to be pulled for letting go to refresh. */
export const PULL_THRESHOLD = 60;
/** The most it follows the finger, and how much of the finger's distance it follows. */
const PULL_MAX = 96;
const RESISTANCE = 0.5;

export interface PullOptions {
  /** Called as the pull moves: how far it is shown (0 once let go), whether that is far enough, and the list's top edge. */
  onpull: (distance: number, ready: boolean, top: number) => void;
  /** Called when let go far enough. The indicator stays (at the threshold) until this settles. */
  onrefresh: () => Promise<unknown>;
  /** Asked at the start of a touch: false (a console, a dialog, a mouse) leaves the page alone. */
  enabled?: () => boolean;
}

export function pullToRefresh(node: HTMLElement, options: PullOptions) {
  let opts = options;
  let startY = 0;
  let tracking = false;
  let distance = 0;
  let refreshing = false;

  const top = () => node.getBoundingClientRect().top;
  const show = (d: number) => {
    distance = d;
    opts.onpull(d, d >= PULL_THRESHOLD, top());
  };

  function start(e: TouchEvent) {
    if (refreshing || e.touches.length !== 1 || node.scrollTop > 0 || opts.enabled?.() === false) return;
    tracking = true;
    startY = e.touches[0].clientY;
  }

  function move(e: TouchEvent) {
    if (!tracking) return;
    const dy = e.touches[0].clientY - startY;
    if (dy <= 0 || node.scrollTop > 0) {
      // Pushing up, or the list has moved: it is a scroll, not a pull.
      if (distance > 0) show(0);
      if (dy <= 0) tracking = false;
      return;
    }
    // Held the page (it would stretch, or the browser would reload): the pull is ours.
    if (e.cancelable) e.preventDefault();
    show(Math.min(dy * RESISTANCE, PULL_MAX));
  }

  async function end() {
    if (!tracking) return;
    tracking = false;
    if (distance < PULL_THRESHOLD) return show(0);
    refreshing = true;
    show(PULL_THRESHOLD);
    try {
      await opts.onrefresh();
    } finally {
      refreshing = false;
      show(0);
    }
  }

  const cancel = () => {
    if (!tracking) return;
    tracking = false;
    show(0);
  };

  node.addEventListener('touchstart', start, { passive: true });
  // Not passive: a pull has to be able to hold the page still.
  node.addEventListener('touchmove', move, { passive: false });
  node.addEventListener('touchend', end);
  node.addEventListener('touchcancel', cancel);
  return {
    update(next: PullOptions) {
      opts = next;
    },
    destroy() {
      node.removeEventListener('touchstart', start);
      node.removeEventListener('touchmove', move);
      node.removeEventListener('touchend', end);
      node.removeEventListener('touchcancel', cancel);
    },
  };
}
