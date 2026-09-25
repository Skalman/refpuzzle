import { t } from "../i18n/index.ts";
import { Dialog } from "./ui/Dialog.tsx";
import { Button, buttonClass } from "./ui/Button.tsx";

export function BackupDialog({
  onExport,
  onImport,
  onSync,
  onClose,
}: {
  onExport: () => void;
  onImport: (e: Event) => void;
  onSync: () => void;
  onClose: () => void;
}) {
  const s = t();
  return (
    <Dialog title={s.backup.button} widthClass="max-w-88" onClose={onClose}>
      <div class="flex flex-col gap-2">
        <Button variant="primary" size="lg" class="w-full" onClick={onSync}>
          {s.sync.title}
        </Button>
        <Button
          variant="primary"
          size="lg"
          class="w-full"
          onClick={() => {
            onClose();
            onExport();
          }}
        >
          {s.backup.downloadBackup}
        </Button>
        <label class={buttonClass({ variant: "primary", size: "lg", class: "w-full" })}>
          {s.backup.uploadBackup}
          <input type="file" accept=".json" class="hidden" onChange={(e) => onImport(e)} />
        </label>
      </div>
    </Dialog>
  );
}
