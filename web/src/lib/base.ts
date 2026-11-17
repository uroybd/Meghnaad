// The folder the app is served from: `/` for a Worker, `/<repo>/` for a static site on GitHub Pages. It is the page's
// <base href> (index.html has `/`; the static deploy writes its own), so the app's addresses (`/tasks/…`, route.ts) are
// the same wherever it is, and only reading and writing the browser's address adds or takes off the folder.

export const BASE = typeof document === 'undefined' ? '/' : new URL(document.baseURI).pathname.replace(/[^/]*$/, '');

/** The app's address for the browser's `location.pathname`. */
export function fromAddress(pathname: string): string {
  return pathname.startsWith(BASE) ? `/${pathname.slice(BASE.length)}` : pathname;
}

/** The browser's address for one of the app's (`/tasks/…`). */
export function toAddress(path: string): string {
  return BASE + path.replace(/^\//, '');
}
