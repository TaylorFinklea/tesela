/**
 * A tiny trailing-edge debouncer (tesela-vp9.2). QueryInput uses this to
 * delay the `parseQueryWithDiagnostics` pass ~150ms after the user stops
 * typing — see the vp9 spec's item 2 (diagnostics run debounced; syntax
 * highlighting itself stays synchronous, driven straight off `tokenize()`).
 */
export type Debouncer<Args extends unknown[]> = {
  /** Schedule `fn(...args)`, cancelling any pending call. */
  call: (...args: Args) => void;
  /** Cancel a pending call, if any. */
  cancel: () => void;
};

export function createDebouncer<Args extends unknown[]>(
  fn: (...args: Args) => void,
  delayMs: number,
): Debouncer<Args> {
  let handle: ReturnType<typeof setTimeout> | null = null;
  return {
    call(...args: Args) {
      if (handle !== null) clearTimeout(handle);
      handle = setTimeout(() => {
        handle = null;
        fn(...args);
      }, delayMs);
    },
    cancel() {
      if (handle !== null) {
        clearTimeout(handle);
        handle = null;
      }
    },
  };
}
