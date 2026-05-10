import { createContext } from "preact";
import { useContext } from "preact/hooks";

export interface HighlightInfo {
  qis: number[];
  oi?: number;
  muteOptions?: boolean;
  noQuestionOutline?: boolean;
}

export const TutorialHighlightCtx = createContext<HighlightInfo | null>(null);

export function useQuestionHighlight(qi: number) {
  const h = useContext(TutorialHighlightCtx);
  if (!h || !h.qis.includes(qi))
    return { highlighted: false, mute: false, highlightedOi: undefined as number | undefined };
  return {
    highlighted: !h.noQuestionOutline,
    mute: !!h.muteOptions,
    highlightedOi: h.oi,
  };
}

export function useOptionHighlight(qi: number, oi: number): boolean {
  const h = useContext(TutorialHighlightCtx);
  return h != null && h.qis.includes(qi) && h.oi === oi;
}
