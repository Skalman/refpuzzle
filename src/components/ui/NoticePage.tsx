import type { ComponentChildren } from "preact";

/** A page holding one centered notice — a heading, a line, a way back. */
export function NoticePage({
  title,
  message,
  children,
}: {
  title?: string;
  message?: string;
  children?: ComponentChildren;
}) {
  return (
    <div class="pt-16 text-center">
      {title && <h1 class="text-hero font-bold opacity-20">{title}</h1>}
      {message && <p class="my-4 text-muted">{message}</p>}
      {children}
    </div>
  );
}
