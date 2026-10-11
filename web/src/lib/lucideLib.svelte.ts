// What is known about Lucide icons right now: the shapes of the ones that were asked for, and (once the picker was
// opened) the names of all of them. Fetching is in `lucide.ts`; this is the state the components read.

import { fetchCatalog, fetchIcon, type Shape } from './lucide';

const KEY = 'tw-web-icons';

function loadSaved(): Record<string, Shape[]> {
  try {
    const v: unknown = JSON.parse(localStorage.getItem(KEY) ?? '{}');
    return v && typeof v === 'object' && !Array.isArray(v) ? (v as Record<string, Shape[]>) : {};
  } catch {
    return {};
  }
}

class LucideLib {
  /** Icons fetched (or remembered from before): name to shapes. */
  shapes = $state<Record<string, Shape[]>>(typeof localStorage === 'undefined' ? {} : loadSaved());
  /** Names the CDN has no icon for. */
  missing = $state<Record<string, true>>({});
  /** Every icon with its search words; null until the picker asked, or when it could not be had. */
  catalog = $state<Record<string, string[]> | null>(null);
  catalogFailed = $state(false);

  #asked = new Set<string>();
  /** The icons task commands use: the only ones remembered in the browser (not everything the picker showed). */
  #keep = new Set<string>();
  #catalog: Promise<void> | null = null;

  /** Make sure this icon is on its way. Asking again for one in flight or already had does nothing. */
  request(name: string): void {
    if (name in this.shapes || name in this.missing || this.#asked.has(name)) return;
    this.#asked.add(name);
    void fetchIcon(name).then((r) => {
      if ('shapes' in r) {
        this.shapes[name] = r.shapes;
        if (this.#keep.has(name)) this.#save();
      } else if ('missing' in r) {
        this.missing[name] = true;
      } else {
        // Not available right now: a later ask tries again.
        this.#asked.delete(name);
      }
    });
  }

  /** These are the icons in use: remember them in the browser, so they draw at once and without the network. */
  keep(names: string[]): void {
    this.#keep = new Set(names);
    this.#save();
  }

  /** Every icon's name, for the picker. */
  loadCatalog(): Promise<void> {
    this.#catalog ??= fetchCatalog().then((c) => {
      this.catalog = c;
      this.catalogFailed = !c;
      if (!c) this.#catalog = null; // try again the next time the picker opens
    });
    return this.#catalog;
  }

  /** Names matching every word of the query, in the name or in the words it is found by; the name's own first. */
  search(query: string): string[] {
    const words = query
      .toLowerCase()
      .split(/[\s-]+/)
      .filter(Boolean);
    const names = Object.keys(this.catalog ?? {}).sort();
    const hits = names.filter((n) =>
      words.every((w) => n.includes(w) || this.catalog![n].some((t) => t.toLowerCase().includes(w))),
    );
    const own = (n: string) => words.every((w) => n.includes(w));
    return [...hits.filter(own), ...hits.filter((n) => !own(n))];
  }

  #save() {
    try {
      const kept = Object.fromEntries([...this.#keep].filter((n) => n in this.shapes).map((n) => [n, this.shapes[n]]));
      localStorage.setItem(KEY, JSON.stringify(kept));
    } catch {
      /* private mode: they are fetched again */
    }
  }
}

export const lucide = new LucideLib();
