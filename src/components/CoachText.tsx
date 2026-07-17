import type { CoachMessage } from "../engine/coach-types.ts";

/**
 * The L1 coach's calm line, shown in the board padding above the grid. The
 * container always renders (reserving height) so appearing/disappearing text
 * never shifts the board; the line itself fades in on change (fade gated to
 * `prefers-reduced-motion: no-preference` — otherwise it just swaps).
 */
export function CoachText({
  message,
  boxRef,
}: {
  message: CoachMessage | null;
  boxRef: { current: HTMLDivElement | null };
}) {
  return (
    <div ref={boxRef} class="coach-text" aria-live="polite">
      {message && (
        <p key={message.text} class={`coach-line coach-${message.tone}`}>
          {message.lead && <span class="coach-lead">{message.lead}</span>}
          {message.text}
        </p>
      )}
    </div>
  );
}
