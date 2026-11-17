// Which build this is: `static` has no Worker behind it (spec 0013): the tasks and the settings are on a git branch,
// reached from the page, and the page is unlocked with the encryption secret. Any other value is a Worker flavour.
export const STATIC = import.meta.env.VITE_FLAVOUR === 'static';
