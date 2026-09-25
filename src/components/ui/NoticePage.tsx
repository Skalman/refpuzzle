import type { ComponentChildren } from "preact";

/** A page holding one centered notice — a heading, a line, a way back. */
export function NoticePage({ children }: { children: ComponentChildren }) {
  return (
    <div class="pt-16 text-center [&>h1]:text-hero [&>h1]:font-bold [&>h1]:opacity-20 [&>p]:my-4 [&>p]:text-muted">
      {children}
    </div>
  );
}
