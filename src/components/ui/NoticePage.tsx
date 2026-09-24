import type { ComponentChildren } from "preact";

/** A page holding one centered notice — a heading, a line, a way back. */
export function NoticePage({ children }: { children: ComponentChildren }) {
  return <div class="not-found">{children}</div>;
}
