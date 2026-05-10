import { useState, useRef, useEffect } from "preact/hooks";
import type { ComponentChildren } from "preact";
import { t } from "../i18n/index.ts";
import { IconShare } from "./Icons.tsx";

interface Props {
  url: string;
  title?: string;
  onClose: () => void;
  installAction?: () => void;
  installMessage?: string;
  children?: ComponentChildren;
}

export function ShareSheet({
  url,
  title,
  onClose,
  installAction,
  installMessage,
  children,
}: Props) {
  const shareTitle = title ?? "Share";
  const s = t();
  const ref = useRef<HTMLDialogElement>(null);
  const [copied, setCopied] = useState(false);
  const canShare = typeof navigator !== "undefined" && !!navigator.share;

  useEffect(() => {
    ref.current?.showModal();
  }, []);

  async function handleShare() {
    try {
      await navigator.share({ title: shareTitle, url });
    } catch {
      /* cancelled */
    }
  }

  async function handleCopy() {
    try {
      await navigator.clipboard.writeText(url);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      /* */
    }
  }

  return (
    <dialog
      ref={ref}
      class="help-panel share-sheet"
      onClose={onClose}
      onClick={(e) => {
        if (e.target === ref.current) onClose();
      }}
    >
      <div class="help-panel-inner">
        <div class="help-panel-header">
          <h3>{shareTitle}</h3>
          <button class="help-close" onClick={onClose} aria-label={s.aria.close}>
            &times;
          </button>
        </div>
        <div
          class="share-sheet-qr"
          ref={(el) => {
            if (!el) return;
            import("./QrCode.tsx").then(({ default: renderQrSvg }) => {
              el.innerHTML = renderQrSvg(url);
            });
          }}
        />
        <div class="share-sheet-url">{url.replace(/^https?:\/\//, "")}</div>
        {installAction && (
          <button class="primary-btn share-sheet-btn" onClick={installAction}>
            {s.install.button}
          </button>
        )}
        {installMessage && <p class="share-sheet-note">{installMessage}</p>}
        <div class="share-sheet-actions">
          {canShare && (
            <button class="primary-btn share-sheet-btn" onClick={handleShare}>
              <IconShare size="0.9em" /> {s.share.share}
            </button>
          )}
          <button class="primary-btn share-sheet-btn" onClick={handleCopy}>
            {copied ? s.share.copied : s.share.copyLink}
          </button>
        </div>
        {children}
      </div>
    </dialog>
  );
}
