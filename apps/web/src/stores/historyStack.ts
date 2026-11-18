export interface HistoryStack<T> {
  past: T[];
  future: T[];
  maxSize: number;
}

export function createHistoryStack<T>(maxSize = 50): HistoryStack<T> {
  return { past: [], future: [], maxSize };
}

export function pushHistory<T>(
  stack: HistoryStack<T>,
  entry: T,
): HistoryStack<T> {
  const nextPast = [...stack.past, entry];
  const trimmed =
    nextPast.length > stack.maxSize
      ? nextPast.slice(nextPast.length - stack.maxSize)
      : nextPast;
  return {
    ...stack,
    past: trimmed,
    future: [],
  };
}

export function stepUndo<T>(
  stack: HistoryStack<T>,
): { stack: HistoryStack<T>; item: T | null } {
  if (stack.past.length === 0) {
    return { stack, item: null };
  }
  const item = stack.past[stack.past.length - 1]!;
  return {
    item,
    stack: {
      ...stack,
      past: stack.past.slice(0, -1),
      future: [item, ...stack.future],
    },
  };
}

export function stepRedo<T>(
  stack: HistoryStack<T>,
): { stack: HistoryStack<T>; item: T | null } {
  if (stack.future.length === 0) {
    return { stack, item: null };
  }
  const item = stack.future[0]!;
  return {
    item,
    stack: {
      ...stack,
      past: [...stack.past, item],
      future: stack.future.slice(1),
    },
  };
}

export function canUndo<T>(stack: HistoryStack<T>): boolean {
  return stack.past.length > 0;
}

export function canRedo<T>(stack: HistoryStack<T>): boolean {
  return stack.future.length > 0;
}
