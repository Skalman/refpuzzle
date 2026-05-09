import type { AnswerLetter, FlatPuzzle, Puzzle } from "./types.ts";
import { LETTERS, letterIdx } from "./types.ts";
import { deduce } from "./deduce.ts";
import type { DeduceResult } from "./deduce.ts";
import { explainDeduce } from "./explain.ts";
import type { ExplainStep } from "./explain.ts";

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
      action: DeduceResult["action"];
      explain: ExplainStep[];
      questionIndex: number;
      optionIndex: number;
      isForce: boolean;
    };

export function collectTutorialSteps(puzzle: Puzzle, fp: FlatPuzzle): TutorialStep[] {
  const n = fp.n;
  const oc = fp.optionCount;
  const phantomMask = 0b11111 & ~((1 << oc) - 1);
  const answers: (AnswerLetter | null)[] = new Array(n).fill(null);
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

  // Deduce steps
  for (let iter = 0; iter < n * 30; iter++) {
    if (answers.slice(0, n).every((a) => a != null)) break;

    const drs = deduce(fp, answers, eliminated);
    if (drs.length === 0) break;

    const explains = drs.map((dr) => {
      try {
        return explainDeduce(puzzle, fp, answers, eliminated, dr);
      } catch {
        return [];
      }
    });

    for (let k = 0; k < drs.length; k++) {
      const dr = drs[k];
      const explain = explains[k];

      if (dr.action.type === "eliminateMulti") {
        for (let i = 0; i < n; i++) {
          if ((dr.action.questionMask >> i) & 1) {
            for (let b = 0; b < oc; b++) {
              if ((dr.action.optionMask >> b) & 1 && !((eliminated[i] >> b) & 1)) {
                steps.push({
                  kind: "deduce",
                  action: dr.action,
                  explain,
                  questionIndex: i,
                  optionIndex: b,
                  isForce: false,
                });
              }
            }
          }
        }
      } else if (dr.action.type === "force") {
        steps.push({
          kind: "deduce",
          action: dr.action,
          explain,
          questionIndex: dr.action.questionIndex,
          optionIndex: letterIdx(dr.action.letter),
          isForce: true,
        });
      } else if (dr.action.type === "eliminate") {
        steps.push({
          kind: "deduce",
          action: dr.action,
          explain,
          questionIndex: dr.action.questionIndex,
          optionIndex: dr.action.optionIndex,
          isForce: false,
        });
      }
    }

    for (const dr of drs) {
      if (dr.action.type === "force") {
        const fOi = letterIdx(dr.action.letter);
        eliminated[dr.action.questionIndex] = 0b11111 ^ (1 << fOi);
        answers[dr.action.questionIndex] = dr.action.letter;
      } else if (dr.action.type === "eliminate") {
        eliminated[dr.action.questionIndex] |= 1 << dr.action.optionIndex;
      } else if (dr.action.type === "eliminateMulti") {
        for (let i = 0; i < n; i++) {
          if ((dr.action.questionMask >> i) & 1) eliminated[i] |= dr.action.optionMask;
        }
      }
    }
  }

  return steps;
}
