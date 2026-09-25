import { useState, useRef, useEffect } from "preact/hooks";
import type { ComponentChildren } from "preact";
import { IconChevronDown } from "./Icons.tsx";
import { menuNavHandler } from "../lib/keyboard.ts";
import { MenuPopover } from "./ui/Menu.tsx";

/**
 * The chevron half of a split button; the host lays it out beside the primary
 * action and styles the chevron through `buttonClass`. Opening from the
 * keyboard focuses the first item; Escape closes and hands focus back; arrow
 * keys stay inside the popup rather than walking the surrounding toolbar.
 *
 * `children` renders the menu's rows (`MenuItem`s) and receives a `close`
 * callback for items that should dismiss on pick.
 */
export function SplitMenu({
  buttonClass,
  tabIndex,
  toolbarItem,
  label,
  children,
}: {
  buttonClass: string;
  tabIndex?: number;
  /** Enrolls the chevron in the arrow-key walk of the toolbar around it. */
  toolbarItem?: boolean;
  label: string;
  children: (close: () => void) => ComponentChildren;
}) {
  const [open, setOpen] = useState(false);
  const dropRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return undefined;
    const close = () => setOpen(false);
    document.addEventListener("click", close);
    return () => document.removeEventListener("click", close);
  }, [open]);

  function items() {
    return Array.from(menuRef.current?.querySelectorAll<HTMLElement>("[data-menu-item]") ?? []);
  }

  function handleDropKeyDown(e: KeyboardEvent) {
    if (e.key === "ArrowDown" || e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      e.stopPropagation();
      setOpen(true);
      requestAnimationFrame(() => items()[0]?.focus());
    } else if (e.key === "Escape" && open) {
      e.preventDefault();
      e.stopPropagation();
      setOpen(false);
    }
  }

  const handleMenuKeyDown = menuNavHandler(items, () => {
    setOpen(false);
    dropRef.current?.focus();
  });

  return (
    <span class="relative flex">
      <button
        ref={dropRef}
        class={buttonClass}
        data-toolbar-item={toolbarItem || undefined}
        tabIndex={tabIndex}
        aria-label={label}
        aria-haspopup="true"
        aria-expanded={open}
        onClick={(e) => {
          e.stopPropagation();
          setOpen((v) => !v);
        }}
        onKeyDown={handleDropKeyDown}
      >
        <IconChevronDown />
      </button>
      {open && (
        <MenuPopover ref={menuRef} aria-label={label} onKeyDown={handleMenuKeyDown}>
          {children(() => setOpen(false))}
        </MenuPopover>
      )}
    </span>
  );
}
