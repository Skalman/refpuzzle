import { useState } from "preact/hooks";
import { exportData, planImport, applyImport } from "../lib/backup.ts";
import type { ImportPlan } from "../lib/backup.ts";
import { t } from "../i18n/index.ts";
import { BackupDialog } from "./BackupDialog.tsx";
import { SyncDialog } from "./SyncDialog.tsx";
import { ImportPreview } from "./ImportPreview.tsx";

function downloadBackup(filename: string) {
  const json = exportData();
  const blob = new Blob([json], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}

/**
 * The backup/sync/import state machine, shared by every page that offers the
 * flow. Pair it with `<BackupDialogs>`, which renders whichever step is open.
 */
export function useBackupFlow(opts?: { onChanged?: () => void }) {
  const s = t();
  const [showBackup, setShowBackup] = useState(false);
  const [showSync, setShowSync] = useState(false);
  const [importPlan, setImportPlan] = useState<ImportPlan | null>(null);

  function openBackup() {
    setShowBackup(true);
  }
  function closeBackup() {
    setShowBackup(false);
  }

  function handleUploadFile(e: Event) {
    setShowBackup(false);
    const input = e.target;
    if (!(input instanceof HTMLInputElement)) return;
    const file = input.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      try {
        if (typeof reader.result !== "string") return;
        setImportPlan(planImport(reader.result));
      } catch (err) {
        alert(s.backup.uploadFailed(err instanceof Error ? err.message : "unknown error"));
      }
    };
    reader.readAsText(file);
    input.value = "";
  }

  function openSync() {
    setShowBackup(false);
    setShowSync(true);
  }
  function closeSync() {
    setShowSync(false);
  }

  function handleSyncReceive(json: string) {
    setShowSync(false);
    try {
      setImportPlan(planImport(json));
    } catch (err) {
      alert(s.backup.uploadFailed(err instanceof Error ? err.message : "unknown error"));
    }
  }

  function confirmUpload() {
    if (!importPlan) return;
    applyImport(importPlan);
    setImportPlan(null);
    opts?.onChanged?.();
  }

  function cancelUpload() {
    setImportPlan(null);
  }

  return {
    showBackup,
    openBackup,
    closeBackup,
    showSync,
    closeSync,
    importPlan,
    handleUploadFile,
    openSync,
    handleSyncReceive,
    confirmUpload,
    cancelUpload,
  };
}

export function BackupDialogs({
  backup,
  exportFilename,
}: {
  backup: ReturnType<typeof useBackupFlow>;
  exportFilename: string;
}) {
  return (
    <>
      {backup.showBackup && (
        <BackupDialog
          onExport={() => downloadBackup(exportFilename)}
          onImport={backup.handleUploadFile}
          onSync={backup.openSync}
          onClose={backup.closeBackup}
        />
      )}
      {backup.showSync && (
        <SyncDialog onImport={backup.handleSyncReceive} onClose={backup.closeSync} />
      )}
      {backup.importPlan && (
        <ImportPreview
          plan={backup.importPlan}
          onConfirm={backup.confirmUpload}
          onCancel={backup.cancelUpload}
        />
      )}
    </>
  );
}
