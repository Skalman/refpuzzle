import type { ComponentChildren } from "preact";
import { t } from "../i18n/index.ts";
import { useShareable } from "../lib/hooks.ts";
import { prettyUrl } from "../lib/share.ts";
import { Dialog } from "./ui/Dialog.tsx";
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
    <Dialog title={shareTitle} class="text-center" onClose={onClose}>
      {controls}
      <QrImage value={url} class="mx-auto my-3 size-32 rounded-sm bg-qr-bg" />
      <div class="mb-3 text-caption break-all text-muted" data-testid="share-url">
        {prettyUrl(url)}
      </div>
      {installAction && (
        <Button variant="primary" onClick={installAction}>
          {s.install.button}
        </Button>
      )}
      {installMessage && <p class="mb-2 text-chrome text-muted">{installMessage}</p>}
      <div class="flex justify-center gap-2">
        {shareable.canShare && (
          <Button variant="primary" icon={<IconShare />} onClick={shareable.share}>
            {s.share.share}
          </Button>
        )}
        <Button variant="primary" onClick={shareable.copy}>
          {shareable.copied ? s.share.copied : s.share.copyLink}
        </Button>
      </div>
      {children}
    </Dialog>
  );
}
