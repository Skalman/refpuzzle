import { useState, useEffect, useRef } from "preact/hooks";
import type { Marks, Puzzle } from "../engine/types.ts";
import { getFlatPuzzle } from "../engine/types.ts";
import { checkAnswerValidity } from "../engine/check-validity.ts";
import { deriveState } from "../engine/state.ts";
import type { Validity } from "../engine/state.ts";
import { collectTutorialSteps } from "../engine/tutorial.ts";
import type { TutorialStep } from "../engine/tutorial.ts";
import type { QuestionState } from "../lib/store.ts";
import type { HighlightInfo } from "./TutorialHighlight.ts";

const FRESH_MARKS: Marks = ["unmarked", "unmarked", "unmarked", "unmarked", "unmarked"];

function cloneStates(qs: QuestionState[]): QuestionState[] {
  return qs.map((q) => ({ marks: [...q.marks] as Marks }));
}

interface UseTutorialOpts {
  questionsRef: { current: QuestionState[] };
  setQuestions: (qs: QuestionState[]) => void;
  setValidity: (v: Validity[]) => void;
  historyRef: { current: QuestionState[][] };
  historyIdxRef: { current: number };
  forceHistoryUpdate: () => void;
  revalidate: (qs: QuestionState[]) => void;
  autoStartTutorial?: boolean;
  onTutorialConsumed?: () => void;
  onStartTutorial?: () => void;
}

export function useTutorial(puzzle: Puzzle, opts: UseTutorialOpts) {
  const isIntro = (puzzle.optionCount ?? 5) < 5;
  const [active, setActive] = useState(false);
  const [welcome, setWelcome] = useState(() => {
    try {
      return !localStorage.getItem("refpuzzle:tutorial-seen");
    } catch {
      return false;
    }
  });
  const [done, setDone] = useState(false);
  const [highlight, setHighlight] = useState<HighlightInfo | null>(null);
  const [steps] = useState<TutorialStep[]>(() =>
    isIntro ? collectTutorialSteps(puzzle, getFlatPuzzle(puzzle)) : [],
  );

  const snapshotsRef = useRef<QuestionState[][]>([]);
  const preTutorialRef = useRef<{
    questions: QuestionState[];
    history: QuestionState[][];
    historyIdx: number;
  } | null>(null);

  const optsRef = useRef(opts);
  optsRef.current = opts;

  function updatePuzzleState(qs: QuestionState[]) {
    optsRef.current.setQuestions(qs);
    const fp = getFlatPuzzle(puzzle);
    const { answers, eliminated } = deriveState(
      qs.map((q) => q.marks),
      puzzle.optionCount,
    );
    optsRef.current.setValidity(
      answers.map((a, qi) =>
        a == null ? "neutral" : checkAnswerValidity(fp, answers, eliminated, qi),
      ),
    );
  }

  function applyStep(step: TutorialStep) {
    if (step.kind !== "deduce") return;
    snapshotsRef.current.push(cloneStates(optsRef.current.questionsRef.current));
    const next = cloneStates(optsRef.current.questionsRef.current);
    if (step.isForce) {
      for (let oi = 0; oi < (puzzle.optionCount ?? 5); oi++) {
        next[step.questionIndex].marks[oi] = oi === step.optionIndex ? "correct" : "incorrect";
      }
    } else {
      next[step.questionIndex].marks[step.optionIndex] = "incorrect";
    }
    updatePuzzleState(next);
  }

  function unapplyStep(_step: TutorialStep) {
    const snap = snapshotsRef.current.pop();
    if (snap) updatePuzzleState(snap);
  }

  function dismiss() {
    setActive(false);
    setHighlight(null);
    setDone(true);
    setTimeout(() => setDone(false), 3000);
    try {
      localStorage.setItem("refpuzzle:tutorial-seen", "1");
    } catch {
      /* */
    }
    const saved = preTutorialRef.current;
    if (saved) {
      optsRef.current.setQuestions(saved.questions);
      optsRef.current.historyRef.current = saved.history;
      optsRef.current.historyIdxRef.current = saved.historyIdx;
      optsRef.current.forceHistoryUpdate();
      optsRef.current.revalidate(saved.questions);
      preTutorialRef.current = null;
    } else {
      const blank = puzzle.questions.map(() => ({ marks: [...FRESH_MARKS] as Marks }));
      optsRef.current.setQuestions(blank);
      optsRef.current.revalidate(blank);
    }
  }

  function startManual() {
    preTutorialRef.current = {
      questions: cloneStates(optsRef.current.questionsRef.current),
      history: optsRef.current.historyRef.current.map((h) => cloneStates(h)),
      historyIdx: optsRef.current.historyIdxRef.current,
    };
    const blank = puzzle.questions.map(() => ({ marks: [...FRESH_MARKS] as Marks }));
    optsRef.current.setQuestions(blank);
    setActive(true);
  }

  function dismissWelcome() {
    setWelcome(false);
    try {
      localStorage.setItem("refpuzzle:tutorial-seen", "1");
    } catch {
      /* */
    }
  }

  function startFromWelcome() {
    setWelcome(false);
    if (isIntro) {
      setActive(true);
    } else {
      optsRef.current.onStartTutorial?.();
    }
  }

  const { autoStartTutorial } = opts;
  useEffect(() => {
    if (autoStartTutorial && !active) {
      setActive(true);
      optsRef.current.onTutorialConsumed?.();
    }
    // oxlint-disable-next-line react-hooks/exhaustive-deps
  }, [active, autoStartTutorial]);

  useEffect(() => {
    if (!welcome || active) return undefined;
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") {
        dismissWelcome();
        e.preventDefault();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // oxlint-disable-next-line react-hooks/exhaustive-deps
  }, [welcome, active]);

  return {
    active,
    welcome,
    done,
    highlight,
    steps,
    isIntro,
    startManual,
    dismissWelcome,
    startFromWelcome,
    applyStep,
    unapplyStep,
    dismiss,
    setHighlight,
  };
}
