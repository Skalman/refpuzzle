import { useRef, useEffect } from "preact/hooks";
import type { ComponentChildren } from "preact";
import { t } from "../i18n/index.ts";
import { useShareable } from "../lib/hooks.ts";
import { prettyUrl } from "../lib/share.ts";
import { Modal } from "./Modal.tsx";
import { IconShare } from "./Icons.tsx";

interface Props {
  url: string;
  title?: string;
  onClose: () => void;
  installAction?: () => void;
  installMessage?: string;
  /** Rendered between the header and the QR code. */
  controls?: ComponentChildren;
  children?: ComponentChildren;
}

export function ShareDialog({
  url,
  title,
  onClose,
  installAction,
  installMessage,
  controls,
  children,
}: Props) {
  const shareTitle = title ?? "Share";
  const s = t();
  const qrRef = useRef<HTMLDivElement>(null);
  const shareable = useShareable({ title: shareTitle, url, text: url });

  useEffect(() => {
    let canceled = false;
    void import("./QrCode.tsx").then(({ default: renderQrSvg }) => {
      if (!canceled && qrRef.current) qrRef.current.innerHTML = renderQrSvg(url);
    });
    return () => {
      canceled = true;
    };
  }, [url]);

  return (
    <Modal title={shareTitle} class="share-dialog" onClose={onClose}>
      {controls}
      <div ref={qrRef} class="share-dialog-qr" />
      <div class="share-dialog-url">{prettyUrl(url)}</div>
      {installAction && (
        <button class="primary-btn share-dialog-btn" onClick={installAction}>
          {s.install.button}
        </button>
      )}
      {installMessage && <p class="share-dialog-note">{installMessage}</p>}
      <div class="share-dialog-actions">
        {shareable.canShare && (
          <button class="primary-btn share-dialog-btn" onClick={shareable.share}>
            <IconShare size="0.9em" /> {s.share.share}
          </button>
        )}
        <button class="primary-btn share-dialog-btn" onClick={shareable.copy}>
          {shareable.copied ? s.share.copied : s.share.copyLink}
        </button>
      </div>
      {children}
    </Modal>
  );
}
