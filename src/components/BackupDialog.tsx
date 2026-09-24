import { t } from "../i18n/index.ts";
import { Modal } from "./ui/Modal.tsx";
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
    <Modal title={s.backup.button} class="sync-dialog" onClose={onClose}>
      <div class="backup-actions">
        <Button variant="primary" class="backup-action-btn" onClick={onSync}>
          {s.sync.title}
        </Button>
        <Button
          variant="primary"
          class="backup-action-btn"
          onClick={() => {
            onClose();
            onExport();
          }}
        >
          {s.backup.downloadBackup}
        </Button>
        <label class={buttonClass("primary", "backup-action-btn")}>
          {s.backup.uploadBackup}
          <input type="file" accept=".json" class="file-input" onChange={(e) => onImport(e)} />
        </label>
      </div>
    </Modal>
  );
}
