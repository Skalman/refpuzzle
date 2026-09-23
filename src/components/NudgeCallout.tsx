import { useEffect, useLayoutEffect, useRef, useState } from "preact/hooks";
import { arcPath } from "../lib/arcPath.ts";
import { classNames } from "../lib/classNames.ts";

/** Gap between the text's underside and the button it names. */
const GAP = 44;
/** How far the text sits to one side, so the arrow leans instead of dropping straight. */
const LEAN = 22;
/** Room kept between the text and the view's edges. */
const EDGE = 8;
/** How long the text itself stays; the arrow outlives it, being out of the way. */
const TEXT_MS = 6_000;
/** The arrow's own fade, run just before the nudge retires. */
const ARROW_FADE_MS = 700;

type ElementRef = { current: HTMLElement | null };

interface Placement {
  x: number;
  y: number;
  path: string;
}

/**
 * The idle nudge: a coach message and arrow, placed just above the button
 * they name, so the arrow is short and angled rather than crossing the board.
 * Floats over everything and takes no pointer events, so it never moves the
 * board or swallows a press.
 */
export function NudgeCallout({
  text,
  targetRef,
  kind,
  showMs,
}: {
  text: string;
  targetRef: ElementRef;
  kind: string;
  /** The nudge's whole life, so the arrow can fade out just before it ends. */
  showMs: number;
}) {
  const hostRef = useRef<HTMLDivElement>(null);
  const textRef = useRef<HTMLParagraphElement>(null);
  const [placement, setPlacement] = useState<Placement | null>(null);
  const [said, setSaid] = useState(false);
  const [going, setGoing] = useState(false);
  // Bumped on resize; the count itself means nothing.
  const [viewport, setViewport] = useState(0);

  useEffect(() => {
    setSaid(false);
    setGoing(false);
    const textTimer = window.setTimeout(() => setSaid(true), TEXT_MS);
    const arrowTimer = window.setTimeout(() => setGoing(true), Math.max(0, showMs - ARROW_FADE_MS));
    return () => {
      clearTimeout(textTimer);
      clearTimeout(arrowTimer);
    };
  }, [kind, showMs]);

  useLayoutEffect(() => {
    const host = hostRef.current;
    const textEl = textRef.current;
    const target = targetRef.current;
    if (!host || !textEl || !target) return;

    const origin = host.getBoundingClientRect();
    const box = target.getBoundingClientRect();
    const buttonX = box.left + box.width / 2 - origin.left;
    const buttonY = box.top - origin.top;

    // Lean toward the middle of the view, so the text has room to sit.
    const side = buttonX > origin.width / 2 ? -1 : 1;
    const half = textEl.offsetWidth / 2;
    let x = buttonX + side * LEAN;
    x = Math.min(Math.max(x, half + EDGE), origin.width - half - EDGE);
    // Never past 45°: a wide clamp would otherwise flatten the arrow out.
    x = Math.min(Math.max(x, buttonX - GAP), buttonX + GAP);
    const y = buttonY - GAP;

    setPlacement({ x, y: y - textEl.offsetHeight, path: arcPath(x, y, buttonX, buttonY, true) });
  }, [targetRef, kind, text, viewport]);

  useLayoutEffect(() => {
    const onResize = () => setViewport((n) => n + 1);
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);

  return (
    <div ref={hostRef} class="nudge-callout">
      <p
        ref={textRef}
        class={classNames("coach-message", "coach-alert", "nudge-text", said && "nudge-text-said")}
        role="status"
        style={
          placement
            ? { left: `${placement.x}px`, top: `${placement.y}px` }
            : { visibility: "hidden" }
        }
      >
        {text}
      </p>
      <svg class={classNames("coach-svg", going && "nudge-arrow-going")} aria-hidden="true">
        {placement && (
          // Keyed on the kind, so switching nudges replays the draw-on.
          <g key={kind}>
            <path class="coach-arrow-outline" d={placement.path} pathLength={100} fill="none" />
            <path class="coach-arrow" d={placement.path} pathLength={100} fill="none" />
          </g>
        )}
      </svg>
    </div>
  );
}
