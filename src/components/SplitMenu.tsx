import { useState, useRef, useEffect } from "preact/hooks";
import type { ComponentChildren } from "preact";
import { IconChevronDown } from "./Icons.tsx";
import { menuNavHandler } from "../lib/keyboard.ts";

/**
 * The chevron half of a split button — put it and the primary action inside a
 * `.split-btn` element. Opening from the keyboard focuses the first item;
 * Escape closes and hands focus back; arrow keys stay inside the popup rather
 * than walking the surrounding toolbar.
 *
 * `children` receives a `close` callback for items that should dismiss on pick.
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
    return Array.from(menuRef.current?.querySelectorAll("button") ?? []);
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
    <span class="split-btn-wrapper">
      <button
        ref={dropRef}
        class={`${buttonClass} split-btn-drop`}
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
        <IconChevronDown size="1em" />
      </button>
      {open && (
        <div
          ref={menuRef}
          class="split-btn-menu"
          role="menu"
          aria-label={label}
          onKeyDown={handleMenuKeyDown}
        >
          {children(() => setOpen(false))}
        </div>
      )}
    </span>
  );
}
