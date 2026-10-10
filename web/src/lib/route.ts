// A task has an address, `/task/<first 8 characters of its uuid>`, so a link can be shared with anyone who can sign
// in to the same deployment. It opens the task's detail drawer. (The first 8 characters are how a task is named
// everywhere else too; the numbers `task 3` uses differ from one replica to the next and change as tasks finish.)

/** The address that opens this task. */
export function taskPath(uuid: string): string {
  return `/task/${uuid.slice(0, 8)}`;
}

/** The uuid characters in an address like `/task/1a2b3c4d`, or `null` when the address is not a task's. */
export function prefixFromPath(path: string): string | null {
  const m = /^\/task\/([0-9a-f][0-9a-f-]{7,35})\/?$/i.exec(path);
  return m ? m[1].toLowerCase() : null;
}

/** Everything to put in a message: the address with the site in front. */
export function taskLink(uuid: string, origin: string): string {
  return `${origin}${taskPath(uuid)}`;
}

/**
 * The filter that finds a task by the start of its uuid. A bare `12345678` would be read as task numbers, so the
 * attribute form is used; it works for any prefix.
 */
export function prefixFilter(prefix: string): string {
  return `uuid.startswith:${prefix}`;
}
