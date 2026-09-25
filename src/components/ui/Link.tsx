import type { AnchorHTMLAttributes } from "preact";
import { classNames } from "../../lib/classNames.ts";

/** A text link in running prose: the accent color, underlined on hover. */
export function Link({
  class: extraClass,
  ...rest
}: Omit<AnchorHTMLAttributes, "class" | "className"> & { class?: string }) {
  return <a class={classNames("text-accent hover:underline", extraClass)} {...rest} />;
}
