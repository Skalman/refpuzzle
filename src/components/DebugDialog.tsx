import { useState } from "preact/hooks";
import { Modal } from "./ui/Modal.tsx";
import { Button } from "./ui/Button.tsx";
import { debugEnabled, nudgeSeconds, setDebugEnabled, setNudgeSeconds } from "../lib/debug.ts";

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
    <Modal title="Debug" onClose={onClose}>
      <p class="debug-hint">For this tab only.</p>

      <label class="debug-row">
        <input
          type="checkbox"
          checked={debug}
          onChange={(e) => setDebug(e.currentTarget.checked)}
        />
        <span>
          Debug mode
          <small>Every hint step at once, and any date opens.</small>
        </span>
      </label>

      <fieldset class="debug-row" aria-label="Idle nudge wait">
        <span>
          Nudge after
          <small>Idle wait before the Checkpoint and Hint callouts.</small>
        </span>
        <span class="debug-choices">
          {NUDGE_CHOICES.map((choice) => (
            <label key={String(choice)} class="debug-choice">
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

      <div class="debug-actions">
        <Button variant="primary" onClick={saveAndReload}>
          Save and reload
        </Button>
      </div>
    </Modal>
  );
}
