import { t } from "../i18n/index.ts";
import type { ImportPlan, ImportAction } from "../lib/backup.ts";
import { Dialog } from "./ui/Dialog.tsx";
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
    <Dialog title={s.backup.uploadPreview} widthClass="max-w-136" onClose={onCancel}>
      <p class="mb-2">{s.backup.puzzlesInBackup(plan.entries.length)}</p>
      {ACTION_ORDER.map((action) => {
        const ids = grouped.get(action);
        if (!ids?.length) return null;
        return (
          <div key={action} class="mb-3">
            <h4 class="mt-4 mb-1 text-body font-bold text-default">
              {s.backup.actions[action]} ({ids.length})
            </h4>
            <ul class="mb-3 ml-5 max-h-24 list-disc overflow-y-auto text-caption leading-normal text-muted">
              {ids.map((id) => (
                <li key={id}>{id}</li>
              ))}
            </ul>
          </div>
        );
      })}
      <div class="mt-4 flex items-center gap-4 border-t pt-3">
        {hasChanges ? (
          <>
            <Button variant="primary" onClick={onConfirm}>
              {s.backup.confirmUpload}
            </Button>
            <button
              class="cursor-pointer border-none bg-transparent p-1 text-section leading-none text-muted hover:text-default"
              onClick={onCancel}
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
    </Dialog>
  );
}
