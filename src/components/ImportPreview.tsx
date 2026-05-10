import { useRef, useEffect } from "preact/hooks";
import { t } from "../i18n/index.ts";
import type { ImportPlan, ImportAction } from "../lib/backup.ts";

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
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    ref.current?.showModal();
  }, []);

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
    <dialog
      ref={ref}
      class="help-panel import-preview"
      onClose={onCancel}
      onClick={(e) => {
        if (e.target === ref.current) onCancel();
      }}
    >
      <div class="help-panel-inner">
        <div class="help-panel-header">
          <h3>{s.backup.importPreview}</h3>
          <button class="help-close" onClick={onCancel} aria-label={s.aria.close}>
            &times;
          </button>
        </div>
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
              <button class="primary-btn" onClick={onConfirm}>
                {s.backup.confirmImport}
              </button>
              <button class="help-close" onClick={onCancel} style={{ fontSize: "0.9rem" }}>
                {s.backup.cancel}
              </button>
            </>
          ) : (
            <button class="primary-btn" onClick={onCancel}>
              {s.backup.ok}
            </button>
          )}
        </div>
      </div>
    </dialog>
  );
}
