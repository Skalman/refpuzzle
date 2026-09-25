import type { AnchorHTMLAttributes, ButtonHTMLAttributes, HTMLAttributes } from "preact";
import { forwardRef } from "preact/compat";
import { classNames, tw } from "../../lib/classNames.ts";

/** A dropdown menu's surface, hanging under the right edge of what opens it. */
export const MenuPopover = forwardRef<
  HTMLDivElement,
  Omit<HTMLAttributes<HTMLDivElement>, "class" | "className">
>(function MenuPopover(props, ref) {
  return (
    <div
      ref={ref}
      role="menu"
      class="absolute top-full right-0 z-20 mt-1 flex min-w-30 flex-col rounded-md border border-strong bg-surface py-1 text-chrome whitespace-nowrap shadow-dropdown"
      {...props}
    />
  );
});

/** A row: roomier below the `md` breakpoint, where it is a touch target. */
const ITEM = tw`w-full cursor-pointer items-center gap-[0.4em] bg-transparent px-3 py-2.5 text-left text-default hover:bg-hover md:py-1.5`;

interface ItemOptions {
  /** Shown only from the `md` breakpoint up. */
  desktopOnly?: boolean;
  /** Shown only below the `md` breakpoint. */
  mobileOnly?: boolean;
  class?: string;
}

function itemClass({ desktopOnly, mobileOnly, class: extraClass }: ItemOptions): string {
  const display = desktopOnly ? "hidden md:flex" : mobileOnly ? "flex md:hidden" : "flex";
  return classNames(ITEM, display, extraClass);
}

/**
 * A row of a menu; `data-menu-item` enrolls it in the menu's arrow keys. The
 * role can be overridden, e.g. for a radio choice.
 */
export const MenuItem = forwardRef<
  HTMLButtonElement,
  Omit<ButtonHTMLAttributes, "class" | "className"> & ItemOptions
>(function MenuItem({ desktopOnly, mobileOnly, class: extraClass, ...rest }, ref) {
  return (
    <button
      ref={ref}
      role="menuitem"
      data-menu-item
      class={itemClass({ desktopOnly, mobileOnly, class: extraClass })}
      {...rest}
    />
  );
});

/** A menu row that navigates. */
export function MenuLink({
  desktopOnly,
  mobileOnly,
  class: extraClass,
  ...rest
}: Omit<AnchorHTMLAttributes, "class" | "className"> & ItemOptions) {
  return (
    <a
      role="menuitem"
      data-menu-item
      class={itemClass({ desktopOnly, mobileOnly, class: extraClass })}
      {...rest}
    />
  );
}
