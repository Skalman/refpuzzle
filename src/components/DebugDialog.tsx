import { useState } from "preact/hooks";
import { Dialog } from "./ui/Dialog.tsx";
import { Button } from "./ui/Button.tsx";
import { debugEnabled, nudgeSeconds, setDebugEnabled, setNudgeSeconds } from "../lib/debug.ts";
import { tw } from "../lib/classNames.ts";

/** One switch per row, ruled off from the one above. */
const ROW = tw`m-0 flex items-start gap-[0.6rem] border-0 border-t py-[0.6rem] text-chrome`;
/** The line under a switch's name saying what it does. */
const NOTE = tw`block text-caption text-muted`;

/** Offered waits for the idle nudge, in seconds; null is the shipped one. */
const NUDGE_CHOICES: (number | null)[] = [null, 3, 10, 30];

/**
 * The development switches. Saving reloads, since their readers read once on
 * mount. Dev-only, so the strings stay here instead of in the i18n.
 */
export function DebugDialog({ onClose }: { onClose: () => void }) {
  const [debug, setDebug] = useState(debugEnabled());
  const [nudge, setNudge] = useState(nudgeSeconds());

  function saveAndReload() {
    setDebugEnabled(debug);
    setNudgeSeconds(nudge);
    window.location.reload();
  }

  return (
    <Dialog title="Debug" onClose={onClose}>
      <p class="mb-2 text-caption text-muted">For this tab only.</p>

      <label class={ROW}>
        <input
          type="checkbox"
          checked={debug}
          onChange={(e) => setDebug(e.currentTarget.checked)}
        />
        <span>
          Debug mode
          <small class={NOTE}>Every hint step at once, and any date opens.</small>
        </span>
      </label>

      <fieldset class={ROW} aria-label="Idle nudge wait">
        <span>
          Nudge after
          <small class={NOTE}>Idle wait before the Checkpoint and Hint callouts.</small>
        </span>
        <span class="ms-auto flex flex-wrap gap-2">
          {NUDGE_CHOICES.map((choice) => (
            <label key={String(choice)} class="flex items-center gap-1 whitespace-nowrap">
              <input
                type="radio"
                name="debug-nudge"
                checked={nudge === choice}
                onChange={() => setNudge(choice)}
              />
              {choice === null ? "default" : `${choice}s`}
            </label>
          ))}
        </span>
      </fieldset>

      <div class="mt-4 flex justify-end border-t pt-3">
        <Button variant="primary" onClick={saveAndReload}>
          Save and reload
        </Button>
      </div>
    </Dialog>
  );
}
