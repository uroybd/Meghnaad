// Which rows of the Tasks table are ticked. Held outside the table so the bulk bar, the table and the console see
// the same ones; the table drops the ones that are no longer in it.

class Selection {
  /** Ticked tasks, in the order they were ticked. */
  uuids = $state<string[]>([]);

  get count(): number {
    return this.uuids.length;
  }

  has(uuid: string): boolean {
    return this.uuids.includes(uuid);
  }

  toggle(uuid: string) {
    this.uuids = this.has(uuid) ? this.uuids.filter((u) => u !== uuid) : [...this.uuids, uuid];
  }

  /** Tick all of these (or none of them, when they are all ticked already). */
  toggleAll(uuids: string[]) {
    const all = uuids.length > 0 && uuids.every((u) => this.has(u));
    this.uuids = all
      ? this.uuids.filter((u) => !uuids.includes(u))
      : [...this.uuids, ...uuids.filter((u) => !this.has(u))];
  }

  clear() {
    if (this.uuids.length) this.uuids = [];
  }

  /** Keep only the ones still in `present`. */
  keep(present: Set<string>) {
    if (this.uuids.some((u) => !present.has(u))) this.uuids = this.uuids.filter((u) => present.has(u));
  }
}

export const selection = new Selection();
