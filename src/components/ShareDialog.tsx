import type { ComponentChildren } from "preact";
import { t } from "../i18n/index.ts";
import { useShareable } from "../lib/hooks.ts";
import { prettyUrl } from "../lib/share.ts";
import { Modal } from "./ui/Modal.tsx";
import { Button } from "./ui/Button.tsx";
import { QrImage } from "./ui/QrImage.tsx";
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
  const shareable = useShareable({ title: shareTitle, url, text: url });

  return (
    <Modal title={shareTitle} class="share-dialog" onClose={onClose}>
      {controls}
      <QrImage value={url} class="share-dialog-qr" />
      <div class="share-dialog-url" data-testid="share-url">
        {prettyUrl(url)}
      </div>
      {installAction && (
        <Button variant="primary" class="share-dialog-btn" onClick={installAction}>
          {s.install.button}
        </Button>
      )}
      {installMessage && <p class="share-dialog-note">{installMessage}</p>}
      <div class="share-dialog-actions">
        {shareable.canShare && (
          <Button variant="primary" class="share-dialog-btn" onClick={shareable.share}>
            <IconShare size="0.9em" /> {s.share.share}
          </Button>
        )}
        <Button variant="primary" class="share-dialog-btn" onClick={shareable.copy}>
          {shareable.copied ? s.share.copied : s.share.copyLink}
        </Button>
      </div>
      {children}
    </Modal>
  );
}
