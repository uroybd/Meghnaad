// Keys for the multi-line text fields (string UDAs and annotations).

/** Submit the form that holds `e`'s field. */
function submit(e: KeyboardEvent) {
  e.preventDefault();
  (e.currentTarget as HTMLTextAreaElement).form?.requestSubmit();
}

/**
 * For a field inside a bigger form (a string UDA): Enter adds a line, as in any text box, and
 * Ctrl/Cmd+Enter saves the form.
 */
export function newlineOnEnter(e: KeyboardEvent) {
  if (e.key === 'Enter' && (e.ctrlKey || e.metaKey) && !e.isComposing) submit(e);
}

/**
 * For a one-line-at-a-time field (an annotation): Enter saves, as it did when this was a single-line
 * input, and Shift+Enter adds a line.
 */
export function submitOnEnter(e: KeyboardEvent) {
  if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) submit(e);
}
