import { t } from "../i18n/index.ts";
import type { ImportPlan, ImportAction } from "../lib/backup.ts";
import { Modal } from "./ui/Modal.tsx";
import { Button } from "./ui/Button.tsx";

export const ACTION_ORDER: ImportAction[] = [
  "new",
  "replace-completed",
  "replace-longer",
  "keep-completed",
  "keep-longer",
  "identical",
];

export function ImportPreview({
  plan,
  onConfirm,
  onCancel,
}: {
  plan: ImportPlan;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const s = t();

  const grouped = new Map<ImportAction, string[]>();
  for (const entry of plan.entries) {
    const list = grouped.get(entry.action) ?? [];
    list.push(entry.id);
    grouped.set(entry.action, list);
  }
  for (const list of grouped.values()) list.sort();
  const hasChanges = plan.entries.some(
    (e) => e.action === "new" || e.action === "replace-completed" || e.action === "replace-longer",
  );

  return (
    <Modal title={s.backup.uploadPreview} class="import-preview" onClose={onCancel}>
      <p class="import-summary">{s.backup.puzzlesInBackup(plan.entries.length)}</p>
      {ACTION_ORDER.map((action) => {
        const ids = grouped.get(action);
        if (!ids?.length) return null;
        return (
          <div key={action} class="import-section">
            <h4>
              {s.backup.actions[action]} ({ids.length})
            </h4>
            <ul class="import-list">
              {ids.map((id) => (
                <li key={id}>{id}</li>
              ))}
            </ul>
          </div>
        );
      })}
      <div class="import-actions">
        {hasChanges ? (
          <>
            <Button variant="primary" onClick={onConfirm}>
              {s.backup.confirmUpload}
            </Button>
            <button
              class="help-close"
              onClick={onCancel}
              style={{ fontSize: "var(--text-section)" }}
            >
              {s.backup.cancel}
            </button>
          </>
        ) : (
          <Button variant="primary" onClick={onCancel}>
            {s.backup.ok}
          </Button>
        )}
      </div>
    </Modal>
  );
}
