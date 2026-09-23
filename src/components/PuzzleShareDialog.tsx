import { useId, useState } from "preact/hooks";
import { t } from "../i18n/index.ts";
import type { SavedState } from "../lib/store.ts";
import { getPuzzleUrl, getShareUrl } from "../lib/share.ts";
import { ShareDialog } from "./ShareDialog.tsx";

export type ShareMode = "puzzle" | "progress" | "app";

const MODES: ShareMode[] = ["app", "puzzle", "progress"];

/**
 * The puzzle page's share dialog, with a switch for what the link opens.
 * `getProgress` is read on every render, so the progress link carries the
 * board as it stands; it returns null on a board with nothing to share, and
 * that mode then drops off the switch.
 */
export function PuzzleShareDialog({
  dateStr,
  level,
  initialMode,
  getProgress,
  onClose,
}: {
  dateStr: string;
  level: number;
  initialMode: ShareMode;
  getProgress: () => SavedState | null;
  onClose: () => void;
}) {
  const s = t();
  const [mode, setMode] = useState<ShareMode>(initialMode);
  // Ties the radios into one group, which is what gives them arrow keys.
  const groupName = useId();
  const progress = getProgress();
  const url =
    mode === "app"
      ? `${window.location.origin}/`
      : mode === "progress" && progress
        ? getShareUrl(dateStr, level, progress)
        : getPuzzleUrl(dateStr, level);
  const modes = MODES.filter((x) => x !== "progress" || progress !== null);

  return (
    <ShareDialog
      url={url}
      title={s.share.share}
      onClose={onClose}
      controls={
        <fieldset class="share-modes" aria-label={s.share.opens}>
          {modes.map((x) => (
            <label key={x} class="share-mode">
              <input
                type="radio"
                name={groupName}
                checked={mode === x}
                onChange={() => setMode(x)}
              />
              {s.share.modes[x]}
            </label>
          ))}
        </fieldset>
      }
    />
  );
}
