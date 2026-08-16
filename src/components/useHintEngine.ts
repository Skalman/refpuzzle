import { useCallback, useRef, useState, useEffect } from "preact/hooks";
import type { Answer, Puzzle } from "../engine/types.ts";
import type { ExplainStep } from "../engine/hint-types.ts";
import { deriveState } from "../engine/state.ts";
import { findMistake } from "../engine/mistake.ts";
import type { Mistake } from "../engine/mistake.ts";
import type { QuestionState } from "../lib/store.ts";
import type { PuzzleHandle } from "../lib/wasm.ts";
import { t } from "../i18n/index.ts";

/**
 * The escalation ladder for a key-wrong mark: vague, then the question, then the
 * option. `lead` opens it, so the caller can frame why it's showing.
 */
function mistakeSteps(mistake: Mistake, lead: string): ExplainStep[] {
  const s = t().mistake;
  return [
    { type: "simple", text: lead },
    { type: "simple", text: s.question(mistake.qi) },
    {
      type: "simple",
      text:
        mistake.kind === "answer"
          ? s.answer(mistake.qi, mistake.answer)
          : s.elim(mistake.qi, mistake.answer),
    },
  ];
}

export function useHintEngine(
  puzzle: Puzzle,
  opts: {
    questionsRef: { current: QuestionState[] };
    debugMode: boolean;
    pushHintMarker: (level: number) => void;
    completed: boolean;
    questions: QuestionState[];
    handleRef: { current: PuzzleHandle | null };
  },
) {
  const [hintText, setHintText] = useState<ExplainStep | null>(null);
  const hintRef = useRef<{ steps: ExplainStep[]; step: number } | null>(null);
  const [debugHints, setDebugHints] = useState<ExplainStep[] | null>(null);
  /** Whether a debug ladder is on screen — the value itself would loop the refresh effect below. */
  const debugShownRef = useRef(false);
  const solutionRef = useRef<(Answer | null)[] | null>(null);

  const optsRef = useRef(opts);
  optsRef.current = opts;

  const showDebugHints = useCallback((steps: ExplainStep[] | null) => {
    debugShownRef.current = steps != null;
    setDebugHints(steps);
  }, []);

  const getSolution = useCallback((): (Answer | null)[] => {
    if (!solutionRef.current) {
      const handle = optsRef.current.handleRef.current;
      if (!handle) return new Array<Answer | null>(puzzle.questions.length).fill(null);
      const t0 = performance.now();
      solutionRef.current = handle.solve();
      console.log(`solve: ${(performance.now() - t0).toFixed(1)}ms`);
    }
    return solutionRef.current;
  }, [puzzle]);

  const findError = useCallback(
    (answers: (Answer | null)[], eliminated: number[]): ExplainStep[] | null => {
      const mistake = findMistake(answers, eliminated, getSolution());
      return mistake ? mistakeSteps(mistake, t().mistake.vague) : null;
    },
    [getSolution],
  );

  const computeHint = useCallback((): { steps: ExplainStep[] } | null => {
    const handle = optsRef.current.handleRef.current;
    if (!handle) return null;
    const markSets = optsRef.current.questionsRef.current.map((q) => q.marks);
    const { answers, eliminated } = deriveState(markSets, puzzle.optionCount);

    const errorSteps = findError(answers, eliminated);
    if (errorSteps) return { steps: errorSteps };

    const step = handle.nextStep(answers, eliminated);
    return step ? { steps: step.explain } : null;
  }, [puzzle, findError]);

  // Debug mode leaves the whole ladder on screen; keep it current as the board
  // changes, but never conjure one that isn't showing.
  useEffect(() => {
    if (!opts.debugMode || !debugShownRef.current || opts.completed) return;
    try {
      showDebugHints(computeHint()?.steps ?? null);
    } catch (e) {
      console.error("Hint error:", e);
    }
  }, [opts.questions, opts.debugMode, opts.completed, computeHint, showDebugHints]);

  function handleHint() {
    const { debugMode, pushHintMarker } = optsRef.current;
    if (!debugMode && hintRef.current && hintRef.current.step < hintRef.current.steps.length - 1) {
      hintRef.current.step++;
      setHintText(hintRef.current.steps[hintRef.current.step]);
      pushHintMarker(hintRef.current.step + 1);
      return;
    }

    let result: { steps: ExplainStep[] };
    try {
      result = computeHint() ?? {
        steps: [{ type: "simple", text: "No obvious next step. Try making an assumption." }],
      };
    } catch (e) {
      console.error("Hint error:", e);
      return;
    }
    if (debugMode) {
      showDebugHints(result.steps);
    } else {
      hintRef.current = { steps: result.steps, step: 0 };
      setHintText(result.steps[0]);
      pushHintMarker(1);
    }
  }

  /**
   * Show a mistake spotted outside the hint flow (a refused checkpoint) in the
   * hint panel, seeding the ladder so "More" walks on to the question and then
   * the option. The opening line is priced by the caller, not as a hint; only
   * escalation past it lands markers.
   */
  function showMistake(mistake: Mistake, lead: string) {
    const steps = mistakeSteps(mistake, lead);
    if (optsRef.current.debugMode) {
      showDebugHints(steps);
      return;
    }
    hintRef.current = { steps, step: 0 };
    setHintText(steps[0]);
  }

  function clear() {
    setHintText(null);
    hintRef.current = null;
  }

  const hasMore =
    hintRef.current != null && hintRef.current.step < hintRef.current.steps.length - 1;

  return { hintText, debugHints, hasMore, handleHint, getSolution, showMistake, clear };
}
