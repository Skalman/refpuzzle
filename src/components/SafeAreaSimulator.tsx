import { useEffect, useState } from "preact/hooks";

/* Dev-only: Ctrl+Shift+S cycles simulated device safe-area insets, drawn as a
 * translucent red frame (also over open dialogs, which sit in the top layer). */

const presets = [
  undefined, // no safe area
  { top: 50, right: 20, bottom: 30, left: 35 },
  { top: 30, right: 35, bottom: 50, left: 20 },
] as const;

export function SafeAreaSimulator() {
  const [presetCounter, setPresetCounter] = useState(0);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.shiftKey && e.key === "S") {
        e.preventDefault();
        setPresetCounter((n) => n + 1);
      }
    };
    addEventListener("keydown", handler);
    return () => removeEventListener("keydown", handler);
  }, []);

  const preset = presets[presetCounter % presets.length];

  if (!preset) return null;

  return (
    <>
      <style>
        {`
        :root {
          --safe-area-inset-top: ${preset.top}px;
          --safe-area-inset-bottom: ${preset.bottom}px;
          --safe-area-inset-left: ${preset.left}px;
          --safe-area-inset-right: ${preset.right}px;
        }
        #safe-area-simulator, dialog[open]::after {
          content: "";
          position: fixed;
          inset: 0;
          border-top: var(--safe-area-inset-top) solid rgba(255, 0, 0, 0.3);
          border-bottom: var(--safe-area-inset-bottom) solid rgba(255, 0, 0, 0.3);
          border-left: var(--safe-area-inset-left) solid rgba(255, 0, 0, 0.3);
          border-right: var(--safe-area-inset-right) solid rgba(255, 0, 0, 0.3);
          pointer-events: none;
          z-index: 99999;
        }
      `}
      </style>
      <div id="safe-area-simulator" />
    </>
  );
}
