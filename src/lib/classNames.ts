/** Joins the truthy parts: `classNames("btn", on && "active")`. */
export function classNames(...parts: (string | false | null | undefined)[]): string {
  return parts.filter(Boolean).join(" ");
}
