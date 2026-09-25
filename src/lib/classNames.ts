/** Joins the truthy parts: `classNames("btn", on && "active")`. */
export function classNames(...parts: (string | false | null | undefined)[]): string {
  return parts.filter(Boolean).join(" ");
}

/**
 * Marks a template as Tailwind classes, for the editor's class support outside
 * `class=` attributes; returns the string unchanged.
 */
export const tw = String.raw;
