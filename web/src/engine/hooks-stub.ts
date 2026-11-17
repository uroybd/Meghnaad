// What `tc-hooks` is in a Worker flavour's build, which runs hooks on the Worker: nothing to load.
export default function init(): Promise<unknown> {
  return Promise.reject(new Error('hooks run on the Worker in this build'));
}
export function check(): string {
  throw new Error('hooks run on the Worker in this build');
}
export class Hooks {
  constructor() {
    throw new Error('hooks run on the Worker in this build');
  }
  run(): Promise<string> {
    return Promise.reject(new Error('hooks run on the Worker in this build'));
  }
  free(): void {}
}
