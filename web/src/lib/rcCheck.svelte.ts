import { onDestroy } from 'svelte';
import { callHelper } from '../engine/helpers';
import { debounce } from './debounce';

type Problem = { file: string; line: number; message: string };

/**
 * What saving the files as typed would find (a line this app can't use, an include with no file, a credential), worked
 * out by the engine once typing stops, for the file shown. Until then, what the server said (`saved`). `files` is every
 * file's text, or null while the file shown is not read yet. Called while the panel starts, which its effect belongs to.
 */
export function rcCheck(
  files: () => Record<string, string> | null,
  current: () => string,
  saved: () => Problem[] | undefined,
) {
  let live = $state<Problem[] | null>(null);
  const checkNow = debounce((all: Record<string, string>) => {
    try {
      live = callHelper<Problem[]>('taskrc_check', all);
    } catch {
      live = null;
    }
  }, 300);
  $effect(() => {
    const all = files();
    if (all) checkNow(all);
  });
  onDestroy(() => checkNow.cancel());
  const mine = $derived(
    (live ?? saved() ?? []).filter((p) => p.file === current()).map((p) => ({ line: p.line, message: p.message })),
  );
  return {
    /** The lines of the file shown with a problem. */
    get mine() {
      return mine;
    },
  };
}
