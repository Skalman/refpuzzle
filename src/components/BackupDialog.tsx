import { useRef, useEffect } from "preact/hooks";
import { t } from "../i18n/index.ts";

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
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    ref.current?.showModal();
  }, []);

  return (
    <dialog
      ref={ref}
      class="help-panel sync-dialog"
      onClose={onClose}
      onClick={(e) => {
        if (e.target === ref.current) onClose();
      }}
    >
      <div class="help-panel-inner">
        <div class="help-panel-header">
          <h3>{s.backup.button}</h3>
          <button class="help-close" onClick={onClose} aria-label={s.aria.close}>
            &times;
          </button>
        </div>
        <div class="backup-actions">
          <button
            class="primary-btn backup-action-btn"
            onClick={() => {
              onClose();
              onExport();
            }}
          >
            {s.backup.exportBackup}
          </button>
          <label class="primary-btn backup-action-btn">
            {s.backup.importBackup}
            <input type="file" accept=".json" class="file-input" onChange={(e) => onImport(e)} />
          </label>
          <button class="primary-btn backup-action-btn" onClick={onSync}>
            {s.sync.title}
          </button>
        </div>
      </div>
    </dialog>
  );
}
