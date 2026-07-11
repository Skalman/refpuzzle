import { useRef, useState, useEffect } from "preact/hooks";
import type { Answer, Puzzle } from "../engine/types.ts";
import { letterIdx } from "../engine/types.ts";
import type { ExplainStep } from "../engine/hint-types.ts";
import { deriveState } from "../engine/state.ts";
import type { QuestionState } from "../lib/store.ts";
import type { PuzzleHandle } from "../lib/wasm.ts";

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
  const solutionRef = useRef<(Answer | null)[] | null>(null);

  const optsRef = useRef(opts);
  optsRef.current = opts;

  function getSolution(): (Answer | null)[] {
    if (!solutionRef.current) {
      const handle = optsRef.current.handleRef.current;
      if (!handle) return new Array<Answer | null>(puzzle.questions.length).fill(null);
      const t0 = performance.now();
      solutionRef.current = handle.solve();
      console.log(`solve: ${(performance.now() - t0).toFixed(1)}ms`);
    }
    return solutionRef.current;
  }

  function findError(answers: (Answer | null)[], eliminated: number[]): ExplainStep[] | null {
    const solution = getSolution();
    const n = puzzle.questions.length;
    for (let qi = 0; qi < n; qi++) {
      const correct = solution[qi];
      if (correct == null) continue;
      const correctOi = letterIdx(correct);
      if (answers[qi] != null && answers[qi] !== correct) {
        return [
          { type: "simple", text: "You made an error." },
          { type: "simple", text: `You made an error in #${qi + 1}.` },
          { type: "simple", text: `#${qi + 1} is not ${answers[qi]} — try a different answer.` },
        ];
      }
      if ((eliminated[qi] >> correctOi) & 1) {
        return [
          { type: "simple", text: "You made an error." },
          { type: "simple", text: `You made an error in #${qi + 1}.` },
          {
            type: "simple",
            text: `You incorrectly eliminated #${qi + 1} option ${correct}.`,
          },
        ];
      }
    }
    return null;
  }

  function computeHint(): { steps: ExplainStep[] } | null {
    const handle = optsRef.current.handleRef.current;
    if (!handle) return null;
    const markSets = optsRef.current.questionsRef.current.map((q) => q.marks);
    const { answers, eliminated } = deriveState(markSets, puzzle.optionCount);

    const errorSteps = findError(answers, eliminated);
    if (errorSteps) return { steps: errorSteps };

    const step = handle.nextStep(answers, eliminated);
    return step ? { steps: step.explain } : null;
  }

  useEffect(() => {
    if (!opts.debugMode || debugHints === null || opts.completed) return;
    try {
      const result = computeHint();
      setDebugHints(result?.steps ?? null);
    } catch (e) {
      console.error("Hint error:", e);
    }
    // oxlint-disable-next-line react-hooks/exhaustive-deps
  }, [opts.questions]);

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
      setDebugHints(result.steps);
    } else {
      hintRef.current = { steps: result.steps, step: 0 };
      setHintText(result.steps[0]);
      pushHintMarker(1);
    }
  }

  function clear() {
    setHintText(null);
    hintRef.current = null;
  }

  const hasMore =
    hintRef.current != null && hintRef.current.step < hintRef.current.steps.length - 1;

  return { hintText, debugHints, hasMore, handleHint, getSolution, clear };
}
