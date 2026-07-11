import { useState, useEffect, useRef } from "preact/hooks";
import type { Marks, Puzzle } from "../engine/types.ts";
import { FRESH_MARKS, getFlatPuzzle } from "../engine/types.ts";
import { V_NEUTRAL } from "../engine/state.ts";
import type { Validity } from "../engine/state.ts";
import { collectTutorialSteps } from "../engine/tutorial.ts";
import type { TutorialStep } from "../engine/tutorial.ts";
import { cloneStates } from "../lib/store.ts";
import type { QuestionState } from "../lib/store.ts";
import type { HighlightInfo } from "./TutorialHighlight.ts";
import type { PuzzleHandle } from "../lib/wasm.ts";
import { wasmReady } from "../lib/wasm.ts";

interface UseTutorialOpts {
  level: number;
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
  handleRef: { current: PuzzleHandle | null };
}

export function useTutorial(puzzle: Puzzle, opts: UseTutorialOpts) {
  const isIntro = opts.level === 1;
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
  // Tutorial steps need wasm `deduce` to plan the scripted sequence; build
  // them once wasm is ready. Intro puzzles are short — the welcome dialog
  // gives wasm plenty of time to finish initializing before the user starts.
  const [steps, setSteps] = useState<TutorialStep[]>([]);
  useEffect(() => {
    if (!isIntro) return undefined;
    let cancelled = false;
    void (async () => {
      await wasmReady();
      if (cancelled) return;
      const handle = optsRef.current.handleRef.current;
      if (!handle) return;
      setSteps(collectTutorialSteps(getFlatPuzzle(puzzle), handle));
    })();
    return () => {
      cancelled = true;
    };
  }, [puzzle, isIntro]);

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
    const handle = optsRef.current.handleRef.current;
    const validity: Validity[] = handle
      ? handle.checkAllAnswers(
          qs.map((q) => q.marks),
          puzzle.optionCount ?? 5,
        )
      : new Array(qs.length).fill(V_NEUTRAL);
    optsRef.current.setValidity(validity);
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
      preTutorialRef.current = {
        questions: cloneStates(optsRef.current.questionsRef.current),
        history: optsRef.current.historyRef.current.map((h) => cloneStates(h)),
        historyIdx: optsRef.current.historyIdxRef.current,
      };
      const blank = puzzle.questions.map(() => ({ marks: [...FRESH_MARKS] as Marks }));
      optsRef.current.setQuestions(blank);
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
  }, [autoStartTutorial]);

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
