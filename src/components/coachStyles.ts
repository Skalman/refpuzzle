import { tw } from "../lib/classNames.ts";

/** The coach's line of text, in the board padding or floated over the board. */
export const COACH_TEXT = tw`m-0 text-body leading-[1.35] text-balance`;

/** The overlay the arrows are drawn into, spanning its host. */
export const COACH_SVG = tw`absolute inset-0 size-full overflow-visible`;

/** Arrows draw on once each (the paths set `pathLength` to 100) and bend smoothly when moved. */
const ARROW_MOTION = tw`motion-safe:animate-coach-draw motion-safe:transition-[d] motion-safe:duration-400 motion-safe:[stroke-dasharray:100] motion-safe:[stroke-dashoffset:100]`;

/** The wider under-stroke that gives the arrow a crisp outline in the light theme. */
export const COACH_ARROW_OUTLINE = tw`fill-none stroke-(--coach-outline) stroke-6 ${ARROW_MOTION}`;

export const COACH_ARROW = tw`fill-none stroke-(--coach-arrow) stroke-2 filter-(--coach-glow) ${ARROW_MOTION}`;
