// Iterative tasks (Taskwarrior 3.6's `iter`): done, one makes the next of its series. The engine checks and computes;
// this only names what the forms offer.

/** An iterative task's types (`iter_type`), as Taskwarrior names them; it also takes `fx`, `f+`, `fp` and `ch`. */
export const ITER_TYPES = ['fixed', 'fixed+', 'chained'];

/** Whether a task is open: pending, or iterative (Taskwarrior's `Task::is_open`). It can be done, started, due. */
export const isOpen = (status: string): boolean => status === 'pending' || status === 'iterative';
