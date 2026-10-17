// A reactive object for a test to change the props of a mounted component with (`$state` only works in a `.svelte.ts` file).
export function reactive<T extends object>(value: T): T {
  const state = $state(value);
  return state;
}
