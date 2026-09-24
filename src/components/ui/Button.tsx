import type { AnchorHTMLAttributes, ButtonHTMLAttributes } from "preact";
import { forwardRef } from "preact/compat";
import { classNames } from "../../lib/classNames.ts";

/**
 * The app's button looks: the accent fill, the accent outline, muted text, a
 * square icon, and the solved green that leads onward.
 */
export type ButtonVariant = "primary" | "outline" | "text" | "icon" | "next";

const VARIANT_CLASS: Record<ButtonVariant, string> = {
  primary: "primary-btn",
  outline: "outline-btn",
  text: "toolbar-accent-btn",
  icon: "toolbar-icon-btn",
  next: "next-puzzle-btn",
};

/** A variant's class plus `extra`, for an element dressed as a button, like a file label. */
export function buttonClass(variant: ButtonVariant, extra?: string): string {
  return classNames(VARIANT_CLASS[variant], extra);
}

type Styled<Attributes> = Omit<Attributes, "class" | "className"> & {
  variant: ButtonVariant;
  class?: string;
};

export const Button = forwardRef<HTMLButtonElement, Styled<ButtonHTMLAttributes>>(function Button(
  { variant, class: extra, ...rest },
  ref,
) {
  return <button ref={ref} class={buttonClass(variant, extra)} {...rest} />;
});

/** A link in a button's clothes. */
export const ButtonLink = forwardRef<HTMLAnchorElement, Styled<AnchorHTMLAttributes>>(
  function ButtonLink({ variant, class: extra, ...rest }, ref) {
    return <a ref={ref} class={buttonClass(variant, extra)} {...rest} />;
  },
);
