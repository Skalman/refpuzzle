import type { CoachMessage } from "../engine/coach-types.ts";
import { COACH_TEXT } from "./coachStyles.ts";
import { tw } from "../lib/classNames.ts";

const TONE: Record<CoachMessage["tone"], string> = {
  calm: tw`text-muted`,
  alert: tw`text-pending`,
};

/**
 * The L1 coach's calm line, shown in the board padding above the grid. The
 * container always renders (reserving height) so appearing/disappearing text
 * never shifts the board; the line itself fades in on change (fade gated to
 * `prefers-reduced-motion: no-preference` — otherwise it just swaps). The
 * fixed height holds three lines of body text at the line's 1.35 line height,
 * plus padding: recompute it if the body size changes.
 */
export function CoachText({
  message,
  boxRef,
}: {
  message: CoachMessage | null;
  boxRef: { current: HTMLDivElement | null };
}) {
  return (
    <div
      ref={boxRef}
      class="mb-1 flex h-17 items-center justify-center px-4 py-1.5 text-center"
      aria-live="polite"
    >
      {message && (
        <p
          key={message.text}
          class={`${COACH_TEXT} max-w-lg motion-safe:animate-coach-fade-in ${TONE[message.tone]}`}
        >
          {message.lead && <span class="block">{message.lead}</span>}
          {message.text}
        </p>
      )}
    </div>
  );
}
