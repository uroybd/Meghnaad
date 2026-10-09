// Whether the page is dark right now: the `data-theme` attribute when it says, else the system setting. Colours
// that Taskwarrior picks for a terminal are drawn differently on a light and a dark page, so they follow this.

function read(): boolean {
  if (typeof document === 'undefined') return false;
  const t = document.documentElement.dataset.theme;
  if (t === 'dark') return true;
  if (t === 'light') return false;
  return typeof matchMedia === 'function' && matchMedia('(prefers-color-scheme: dark)').matches;
}

class Scheme {
  dark = $state(read());

  constructor() {
    if (typeof document === 'undefined') return;
    const update = () => (this.dark = read());
    if (typeof matchMedia === 'function')
      matchMedia('(prefers-color-scheme: dark)').addEventListener?.('change', update);
    if (typeof MutationObserver === 'function') {
      new MutationObserver(update).observe(document.documentElement, {
        attributes: true,
        attributeFilter: ['data-theme'],
      });
    }
  }
}

export const scheme = new Scheme();
