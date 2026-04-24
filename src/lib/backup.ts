const PREFIX = "refpuzzle:puzzle:";
const BACKUP_VERSION = 1;

interface BackupData {
  version: number;
  exportedAt: string;
  puzzles: Record<string, string>;
}

export type ImportAction = "new" | "replace-completed" | "replace-longer" | "keep-completed" | "keep-longer" | "identical";

export interface ImportEntry {
  id: string;
  incoming: string;
  existing: string | null;
  action: ImportAction;
}

export interface ImportPlan {
  entries: ImportEntry[];
}

export function exportData(): string {
  const ids: string[] = [];
  for (let i = 0; i < localStorage.length; i++) {
    const key = localStorage.key(i);
    if (key?.startsWith(PREFIX)) ids.push(key.slice(PREFIX.length));
  }
  ids.sort();
  const puzzles: Record<string, string> = {};
  for (const id of ids) {
    const val = localStorage.getItem(PREFIX + id);
    if (val) puzzles[id] = val;
  }
  const data: BackupData = {
    version: BACKUP_VERSION,
    exportedAt: new Date().toISOString(),
    puzzles,
  };
  return JSON.stringify(data, null, 2);
}

function isCompleted(val: string): boolean {
  return val.endsWith(".x") || val === "x";
}

function stepCount(val: string): number {
  return val.split(".").length;
}

export function planImport(json: string): ImportPlan {
  const data: unknown = JSON.parse(json);
  if (!isBackupData(data)) throw new Error("Invalid backup file");
  if (data.version > BACKUP_VERSION) throw new Error(`Unsupported version ${String(data.version)}`);

  const entries: ImportEntry[] = [];
  for (const [id, val] of Object.entries(data.puzzles)) {
    if (typeof val !== "string") continue;
    const existing = localStorage.getItem(PREFIX + id);

    let action: ImportAction;
    if (!existing) {
      action = "new";
    } else if (existing === val) {
      action = "identical";
    } else if (isCompleted(val) && !isCompleted(existing)) {
      action = "replace-completed";
    } else if (isCompleted(existing)) {
      action = "keep-completed";
    } else if (stepCount(val) > stepCount(existing)) {
      action = "replace-longer";
    } else {
      action = "keep-longer";
    }

    entries.push({ id, incoming: val, existing, action });
  }

  return { entries };
}

export function applyImport(plan: ImportPlan): { imported: number; replaced: number; skipped: number } {
  let imported = 0;
  let replaced = 0;
  let skipped = 0;

  for (const entry of plan.entries) {
    if (entry.action === "new") {
      localStorage.setItem(PREFIX + entry.id, entry.incoming);
      imported++;
    } else if (entry.action === "replace-completed" || entry.action === "replace-longer") {
      localStorage.setItem(PREFIX + entry.id, entry.incoming);
      replaced++;
    } else {
      skipped++;
    }
  }

  return { imported, replaced, skipped };
}

function isBackupData(v: unknown): v is BackupData {
  if (!v || typeof v !== "object") return false;
  if (!("version" in v) || typeof v.version !== "number") return false;
  if (!("puzzles" in v) || typeof v.puzzles !== "object" || v.puzzles === null) return false;
  return true;
}
