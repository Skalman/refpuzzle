import { useRef, useEffect, useId } from "preact/hooks";
import type { ComponentChildren } from "preact";
import { classNames } from "../../lib/classNames.ts";
import { CloseButton } from "./CloseButton.tsx";

/**
 * The help-panel shell: title, close button, body, dismissed from the backdrop,
 * Escape or the ×. `class` names the panel for whatever it is holding.
 */
export function Modal({
  title,
  class: extraClass,
  onClose,
  children,
}: {
  title: string;
  class?: string;
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
      class={classNames("help-panel", extraClass)}
      aria-labelledby={titleId}
      onClose={onClose}
      onClick={(e) => {
        if (e.target === ref.current) onClose();
      }}
    >
      <div class="help-panel-inner">
        <div class="help-panel-header">
          <h3 id={titleId}>{title}</h3>
          <CloseButton onClick={onClose} />
        </div>
        {children}
      </div>
    </dialog>
  );
}
