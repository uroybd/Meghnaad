// A yes-or-no question the page asks in its own dialog (AskDialog.svelte, mounted once by main.ts), never the browser's
// `confirm`. Questions asked while one is open wait their turn.

interface Question {
  message: string;
  yes: string;
  no: string;
  resolve: (yes: boolean) => void;
}

class Asker {
  /** The question on screen, if any. */
  current = $state<Question | null>(null);
  #waiting: Question[] = [];

  ask(message: string, words: { yes?: string; no?: string } = {}): Promise<boolean> {
    return new Promise((resolve) => {
      const q = { message, yes: words.yes ?? 'Yes', no: words.no ?? 'No', resolve };
      if (this.current) this.#waiting.push(q);
      else this.current = q;
    });
  }

  /** Answer the question on screen; the next one waiting, if any, takes its place. */
  answer(yes: boolean) {
    const q = this.current;
    if (!q) return;
    this.current = this.#waiting.shift() ?? null;
    q.resolve(yes);
  }
}

export const asker = new Asker();

/** Ask `message` in the page's own dialog: true for yes. */
export const ask = (message: string, words?: { yes?: string; no?: string }) => asker.ask(message, words);
