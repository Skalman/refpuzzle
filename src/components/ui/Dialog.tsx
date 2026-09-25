import { useRef, useEffect, useId } from "preact/hooks";
import type { ComponentChildren } from "preact";
import { classNames } from "../../lib/classNames.ts";
import { CloseButton } from "./CloseButton.tsx";

/**
 * The dialog shell: title, close button, body, dismissed from the backdrop,
 * Escape or the ×. `widthClass` replaces the default max width; `class` adds
 * to the dialog, `titleClass` to its heading.
 */
export function Dialog({
  title,
  widthClass = "max-w-96",
  class: extraClass,
  titleClass,
  onClose,
  children,
}: {
  title: string;
  widthClass?: string;
  class?: string;
  titleClass?: string;
  onClose: () => void;
  children: ComponentChildren;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  const titleId = useId();
  useEffect(() => {
    ref.current?.showModal();
  }, []);

  return (
    <dialog
      ref={ref}
      class={classNames(
        "m-auto max-h-[80vh] w-[calc(100%-1rem)] overflow-y-auto rounded-xl border border-solid border-strong bg-surface p-0 text-default shadow-dialog backdrop:bg-backdrop",
        widthClass,
        extraClass,
      )}
      aria-labelledby={titleId}
      onClose={onClose}
      onClick={(e) => {
        if (e.target === ref.current) onClose();
      }}
    >
      <div class="p-safe-dialog leading-[1.6] text-muted">
        <div class="mb-4 flex items-center justify-between">
          <h3 id={titleId} class={classNames("text-dialog font-bold", titleClass)}>
            {title}
          </h3>
          <CloseButton onClick={onClose} />
        </div>
        {children}
      </div>
    </dialog>
  );
}
