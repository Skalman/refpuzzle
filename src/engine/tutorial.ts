import type { Answer, FlatPuzzle } from "./types.ts";
import { LETTERS, letterIdx } from "./types.ts";
import type { DeduceAction, ExplainStep } from "./hint-types.ts";
import type { PuzzleHandle } from "../lib/wasm.ts";

export type TutorialStep =
  | {
      kind: "intro";
      heading: string;
      text: string;
      highlightQis: number[];
      highlightOi?: number;
      muteOptions?: boolean;
      noQuestionOutline?: boolean;
      durationMs?: number;
      introStep: number;
      introTotal: number;
    }
  | {
      kind: "deduce";
      action: DeduceAction;
      explain: ExplainStep[];
      questionIndex: number;
      optionIndex: number;
      isForce: boolean;
    };

export function collectTutorialSteps(fp: FlatPuzzle, handle: PuzzleHandle): TutorialStep[] {
  const n = fp.n;
  const oc = fp.optionCount;
  const phantomMask = 0b11111 & ~((1 << oc) - 1);
  const answers: (Answer | null)[] = new Array(n).fill(null);
  const eliminated: number[] = new Array(n).fill(phantomMask);
  const steps: TutorialStep[] = [];

  // Intro steps (3 logical steps, each with sub-step animations)
  const allQis = Array.from({ length: n }, (_, i) => i);
  const introTotal = 3;
  for (let qi = 0; qi < n; qi++) {
    steps.push({
      kind: "intro",
      heading: `${n} questions`,
      text: `This puzzle has ${n} questions: ${allQis.map((i) => `#${i + 1}`).join(", ")}.`,
      highlightQis: [qi],
      introStep: 1,
      introTotal,
    });
  }
  for (let oi = 0; oi < oc; oi++) {
    steps.push({
      kind: "intro",
      heading: `${oc} options`,
      text: `Each question has ${oc} options: ${LETTERS.slice(0, oc).join(", ")}.`,
      highlightQis: allQis,
      highlightOi: oi,
      muteOptions: true,
      noQuestionOutline: true,
      introStep: 2,
      introTotal,
    });
  }
  steps.push({
    kind: "intro",
    heading: "Let's go!",
    text: "In a self-referential puzzle, each question refers to the puzzle itself. Let's try it out!",
    highlightQis: allQis,
    noQuestionOutline: true,
    durationMs: 3000,
    introStep: 3,
    introTotal,
  });

  // Deduce steps — one action per iteration to avoid duplicates
  for (let iter = 0; iter < n * 30; iter++) {
    if (answers.every((a) => a != null)) break;

    // The next solving step: a deduction if available, else a lookahead
    // elimination (see `Puzzle::next_step`). Null once the puzzle is solved.
    const step = handle.nextStep(answers, eliminated);
    if (!step) break;
    const { action, explain } = step;

    if (action.type === "eliminateMulti") {
      for (let i = 0; i < n; i++) {
        if ((action.questionMask >> i) & 1) {
          for (let b = 0; b < oc; b++) {
            if ((action.optionMask >> b) & 1 && !((eliminated[i] >> b) & 1)) {
              steps.push({
                kind: "deduce",
                action,
                explain,
                questionIndex: i,
                optionIndex: b,
                isForce: false,
              });
            }
          }
        }
      }
      for (let i = 0; i < n; i++) {
        if ((action.questionMask >> i) & 1) eliminated[i] |= action.optionMask;
      }
    } else if (action.type === "force") {
      steps.push({
        kind: "deduce",
        action,
        explain,
        questionIndex: action.qi,
        optionIndex: letterIdx(action.answer),
        isForce: true,
      });
      const fOi = letterIdx(action.answer);
      eliminated[action.qi] = 0b11111 ^ (1 << fOi);
      answers[action.qi] = action.answer;
    } else if (action.type === "eliminate") {
      steps.push({
        kind: "deduce",
        action,
        explain,
        questionIndex: action.qi,
        optionIndex: action.oi,
        isForce: false,
      });
      eliminated[action.qi] |= 1 << action.oi;
    }
  }

  return steps;
}
