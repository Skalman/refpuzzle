import { useState, useCallback } from "preact/hooks";

export function useForceUpdate(): () => void {
  const [, set] = useState(0);
  return useCallback(() => set((v) => v + 1), []);
}
