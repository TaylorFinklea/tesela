export type DeferredEditorLifecycleOptions<T> = {
  queue: (task: () => void) => void;
  isCurrent: (target: T) => boolean;
  isFocused: (target: T) => boolean;
  clearOwnership: (target: T) => void;
  applyFocus: (target: T) => void;
  applyBlur: (target: T) => void;
};

export type DeferredEditorLifecycle<T> = {
  focus: (target: T) => void;
  blur: (target: T) => void;
  teardown: (target: T) => void;
};

let nextFocusOwnerId = 0;

export function createEditorFocusOwnerId(editorKey: string): string {
  nextFocusOwnerId += 1;
  return `${editorKey}:focus-owner-${nextFocusOwnerId}`;
}

export function createDeferredEditorLifecycle<T>(
  options: DeferredEditorLifecycleOptions<T>,
): DeferredEditorLifecycle<T> {
  let generation = 0;
  let disposed = false;

  return {
    focus(target) {
      if (disposed) return;
      const scheduledGeneration = ++generation;
      options.queue(() => {
        if (disposed || scheduledGeneration !== generation) return;
        if (!options.isCurrent(target) || !options.isFocused(target)) return;
        options.applyFocus(target);
      });
    },

    blur(target) {
      if (disposed) return;
      const scheduledGeneration = ++generation;
      options.queue(() => {
        if (disposed || scheduledGeneration !== generation) return;
        options.clearOwnership(target);
        if (!options.isCurrent(target) || options.isFocused(target)) return;
        options.applyBlur(target);
      });
    },

    teardown(target) {
      if (disposed) return;
      disposed = true;
      generation += 1;
      options.queue(() => options.clearOwnership(target));
    },
  };
}
