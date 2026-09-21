import { useRef, useEffect, useId } from "preact/hooks";
import type { ComponentChildren } from "preact";
import { t } from "../i18n/index.ts";
import { classNames } from "../lib/classNames.ts";

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
  const s = t();
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
          <button class="help-close" onClick={onClose} aria-label={s.aria.close}>
            &times;
          </button>
        </div>
        {children}
      </div>
    </dialog>
  );
}
