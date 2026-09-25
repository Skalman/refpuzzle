import { tw } from "../../lib/classNames.ts";

/** Each level's color as `--level-color`, in tab order: the tabs and the level rail read it. */
export const LEVEL_COLOR = [
  tw`[--level-color:var(--level-color-intro)]`,
  tw`[--level-color:var(--level-color-beginner)]`,
  tw`[--level-color:var(--level-color-easy)]`,
  tw`[--level-color:var(--level-color-medium)]`,
  tw`[--level-color:var(--level-color-hard)]`,
  tw`[--level-color:var(--level-color-expert)]`,
];
