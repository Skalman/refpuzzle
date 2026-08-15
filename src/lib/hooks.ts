import { useState, useCallback, useEffect } from "preact/hooks";
import { onRevalidated } from "./revalidate.ts";

export function useForceUpdate(): () => void {
  const [, set] = useState(0);
  return useCallback(() => set((v) => v + 1), []);
}

/** Re-render when the boot re-check rewrites a stored outcome. */
export function useRevalidated(): void {
  const forceUpdate = useForceUpdate();
  useEffect(() => onRevalidated(forceUpdate), [forceUpdate]);
}
