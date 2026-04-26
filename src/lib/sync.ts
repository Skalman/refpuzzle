import { exportData } from "./backup.ts";

export async function startSync(): Promise<string> {
  const body = exportData();
  const resp = await fetch("/sync.php", { method: "POST", body });
  if (!resp.ok) {
    const data: { error?: string } = await resp.json();
    throw new Error(data.error ?? "Sync failed");
  }
  const data: { code: string } = await resp.json();
  return data.code;
}

export async function joinSync(code: string): Promise<string> {
  const body = exportData();
  const joinResp = await fetch(`/sync.php?code=${code}&side=b`, { method: "POST", body });
  if (!joinResp.ok) {
    const data: { error?: string } = await joinResp.json();
    throw new Error(data.error ?? "Sync failed");
  }
  const dataResp = await fetch(`/sync.php?code=${code}&side=a`);
  if (!dataResp.ok) throw new Error("Could not fetch partner data");
  return dataResp.text();
}

export async function pollSync(code: string): Promise<string | null> {
  const resp = await fetch(`/sync.php?code=${code}&side=b`);
  if (resp.status === 404) return null;
  if (!resp.ok) return null;
  return resp.text();
}
