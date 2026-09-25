import type { AnchorHTMLAttributes, ButtonHTMLAttributes, ComponentChildren } from "preact";
import { forwardRef } from "preact/compat";
import { classNames, tw } from "../../lib/classNames.ts";

/**
 * The app's button looks: the accent fill; the solved green that leads onward,
 * and its muted copy for while the solved dialog carries the loud one; the
 * accent outline, and its muted copy; and bare muted text.
 */
export type ButtonVariant =
  | "primary"
  | "next"
  | "next-muted"
  | "outline"
  | "outline-muted"
  | "ghost";

const FILLED = tw`cursor-pointer rounded-md font-semibold whitespace-nowrap`;
const MUTED = tw`cursor-pointer rounded-md bg-transparent text-muted hover:not-disabled:bg-hover hover:not-disabled:text-default disabled:cursor-default`;

/**
 * Colors, border and shape: everything but the size. Borderless looks leave
 * the border alone, so a caller can add one (a divider, a ring).
 */
const LOOK: Record<ButtonVariant, string> = {
  primary: tw`${FILLED} shrink-0 bg-accent text-on-accent hover:opacity-90`,
  next: tw`${FILLED} border-2 border-valid bg-valid-fill text-default hover:opacity-90`,
  "next-muted": tw`${FILLED} border-2 bg-surface text-muted hover:opacity-90`,
  outline: tw`${FILLED} border border-accent bg-transparent text-accent hover:bg-accent hover:text-on-accent`,
  "outline-muted": tw`${MUTED} border disabled:opacity-30`,
  ghost: tw`${MUTED} disabled:opacity-35`,
};

/**
 * Small for inline follow-ups, large for a dialog's lead or rows, icon for a
 * square glyph button; `md-compact` is `md` with tight padding, for the header's
 * tight controls.
 */
export type ButtonSize = "sm" | "md" | "md-compact" | "lg" | "icon";

/** Every button lays its text and icon out the same way. */
const LAYOUT = tw`inline-flex items-center justify-center gap-[0.3em]`;

/** Type size and padding. */
const SIZE: Record<ButtonSize, string> = {
  sm: tw`px-2 py-0.5 text-caption`,
  md: tw`px-3 py-1.5 text-chrome`,
  "md-compact": tw`px-1.5 py-1.5 text-chrome`,
  lg: tw`px-5 py-2.5 text-section`,
  icon: tw`size-8 text-section`,
};

interface Styling {
  variant: ButtonVariant;
  size?: ButtonSize;
  /** Added to the look and size; for what they leave alone, like width, margins or a shadow. */
  class?: string;
}

/** The classes for a variant, for an element dressed as a button, like a file label. */
export function buttonClass({ variant, size = "md", class: extraClass }: Styling): string {
  return classNames(LAYOUT, LOOK[variant], SIZE[size], extraClass);
}

type Styled<Attributes> = Omit<Attributes, "class" | "className" | "size" | "icon"> &
  Styling & {
    /** Leads the label, at its default size (the text's own); alone, it is the whole button. */
    icon?: ComponentChildren;
  };

export const Button = forwardRef<HTMLButtonElement, Styled<ButtonHTMLAttributes>>(function Button(
  { variant, size, class: extraClass, icon, children, ...rest },
  ref,
) {
  return (
    <button ref={ref} class={buttonClass({ variant, size, class: extraClass })} {...rest}>
      {icon}
      {children}
    </button>
  );
});

/** A link in a button's clothes. */
export const ButtonLink = forwardRef<HTMLAnchorElement, Styled<AnchorHTMLAttributes>>(
  function ButtonLink({ variant, size, class: extraClass, icon, children, ...rest }, ref) {
    return (
      <a ref={ref} class={buttonClass({ variant, size, class: extraClass })} {...rest}>
        {icon}
        {children}
      </a>
    );
  },
);
