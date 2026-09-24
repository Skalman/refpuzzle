import type { ComponentChildren } from "preact";

/** The page-level wait: a spinner, or a line of text in its place. */
export function Loading({ children }: { children?: ComponentChildren }) {
  return <div class="loading">{children ?? <span class="spinner" />}</div>;
}
