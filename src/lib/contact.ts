/**
 * The site's contact mailbox, from `VITE_CONTACT_ADDRESS` at build time. Null
 * when the build set none, and the pages offering it then leave it out.
 */
export function contactAddress(): string | null {
  const configured: unknown = import.meta.env.VITE_CONTACT_ADDRESS;
  return typeof configured === "string" && configured !== "" ? configured : null;
}
