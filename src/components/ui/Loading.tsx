import type { ComponentChildren } from "preact";

/** The page-level wait: a spinner, or a line of text in its place. */
export function Loading({ children }: { children?: ComponentChildren }) {
  return (
    <div class="flex justify-center p-8">
      {children ?? (
        <span class="size-6 animate-spinner rounded-full border-2 border-solid border-t-accent opacity-0" />
      )}
    </div>
  );
}
