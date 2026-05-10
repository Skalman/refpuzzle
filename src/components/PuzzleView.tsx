import { useState, useEffect, useCallback, useRef } from "preact/hooks";
import { tinykeys } from "tinykeys";
import type { AnswerLetter, Marks, Puzzle } from "../engine/types.ts";
import { LETTERS, letterIdx, getFlatPuzzle } from "../engine/types.ts";
import { checkAnswerValidity } from "../engine/check-validity.ts";
import { deduce } from "../engine/deduce.ts";
import { lookaheadShortest } from "../engine/lookahead.ts";
import { solvePuzzle } from "../engine/solve.ts";
import { explainDeduce, explainLookahead } from "../engine/explain.ts";
import type { ExplainStep } from "../engine/explain.ts";
import { deriveState } from "../engine/state.ts";
import type { Validity } from "../engine/state.ts";
import { loadState, saveState, loadMeta, saveMeta, clearMeta } from "../lib/store.ts";
import type { QuestionState, PuzzleMeta } from "../lib/store.ts";
import { decodeShareHash, getShareUrl, getPuzzleUrl, sharePuzzleLink } from "../lib/share.ts";
import { guarded, arrowNavHandler, initRovingTabindex } from "../lib/keyboard.ts";
import { confetti } from "../lib/confetti.ts";
import { track, getClientInfo } from "../lib/analytics.ts";
import { t } from "../i18n/index.ts";
import { QuestionRow } from "./QuestionRow.tsx";
import { TutorialOverlay } from "./TutorialOverlay.tsx";
import { TutorialWelcome } from "./TutorialWelcome.tsx";
import { TutorialHighlightCtx } from "./TutorialHighlight.ts";
import { useTutorial } from "./useTutorial.ts";
import {
  IconUndo,
  IconRedo,
  IconPin,
  IconHint,
  IconCheck,
  IconX,
  IconChevronDown,
  IconReset,
  IconShare,
  IconPlay,
} from "./Icons.tsx";
import type { Ref } from "preact";

const FRESH_MARKS: Marks = ["unmarked", "unmarked", "unmarked", "unmarked", "unmarked"];

function cloneStates(qs: QuestionState[]): QuestionState[] {
  return qs.map((q) => ({ marks: [...q.marks] as Marks }));
}

interface MoveInfo {
  text: string;
  icon: string;
  qi: number;
  oi: number;
  hint?: number;
}

function describeDiff(prev: QuestionState[], next: QuestionState[]): MoveInfo {
  let best: MoveInfo | null = null;
  let bestPriority = -1;
  for (let qi = 0; qi < prev.length; qi++) {
    for (let oi = 0; oi < 5; oi++) {
      const p = prev[qi].marks[oi];
      const n = next[qi].marks[oi];
      if (p === n) continue;
      const letter = LETTERS[oi];
      let priority: number;
      let text: string;
      let icon: string;
      if (n === "correct") {
        text = `#${qi + 1}=${letter}`;
        icon = "ok";
        priority = 2;
      } else if (n === "incorrect") {
        text = `#${qi + 1} ${letter}`;
        icon = "no";
        priority = 1;
      } else {
        text = `#${qi + 1} ${letter}`;
        icon = "un";
        priority = 0;
      }
      if (priority > bestPriority) {
        best = { text, icon, qi, oi };
        bestPriority = priority;
      }
    }
  }
  if (!best) {
    return { text: "", icon: "pin", qi: -1, oi: -1 };
  }
  return best;
}

function HistoryStrip({
  history,
  currentIdx,
  hints,
  completed,
  onJump,
  containerRef,
}: {
  history: QuestionState[][];
  currentIdx: number;
  hints: Map<number, number>;
  completed: boolean;
  onJump: (idx: number) => void;
  containerRef?: { current: HTMLDivElement | null };
}) {
  const s = t();
  if (history.length <= 1) return null;

  const moves: MoveInfo[] = [];
  for (let i = 1; i < history.length; i++) {
    moves.push(describeDiff(history[i - 1], history[i]));
  }

  let lastCp = -1;
  if (!completed)
    for (let i = Math.min(currentIdx - 1, moves.length - 1); i >= 0; i--) {
      if (moves[i].qi < 0 && moves[i].icon) {
        lastCp = i;
        break;
      }
    }

  return (
    <div
      ref={containerRef}
      class="history-strip"
      role="toolbar"
      onKeyDown={arrowNavHandler("button.history-step:not(:disabled)")}
    >
      <button
        class={`history-step ${currentIdx === 0 ? "current" : ""}`}
        onClick={completed ? undefined : () => onJump(0)}
        disabled={completed}
      >
        <span class="history-icon">
          <IconPlay size="1em" />
        </span>{" "}
        {s.puzzle.start}
      </button>
      {moves.map((move, i) => {
        const stepIdx = i + 1;
        const hintLevel = hints.get(stepIdx);
        const isCheckpoint = move.qi < 0 && move.icon;
        return (
          // oxlint-disable-next-line react/no-array-index-key
          <span key={i} class="history-entry">
            <button
              class={`history-step ${!completed && stepIdx === currentIdx ? "current" : ""} ${stepIdx > currentIdx ? "future" : ""} ${isCheckpoint && i === lastCp ? "checkpoint" : ""} ${isCheckpoint && i !== lastCp ? "checkpoint-old" : ""}`}
              onClick={completed ? undefined : () => onJump(stepIdx)}
              disabled={completed}
              title={move.text}
            >
              {move.icon === "pin" && (
                <span class="history-icon">
                  <IconPin size="1.1em" />{" "}
                </span>
              )}
              {move.icon === "ok" && (
                <span class="history-icon icon-correct">
                  <IconCheck size="1.5em" strokeWidth={3} />{" "}
                </span>
              )}
              {move.icon === "no" && (
                <span class="history-icon icon-incorrect">
                  <IconX size="1.5em" strokeWidth={3} />{" "}
                </span>
              )}
              {move.icon === "un" && (
                <span class="history-icon">
                  <IconUndo size="1.5em" strokeWidth={3} />{" "}
                </span>
              )}
              {move.text}
            </button>
            {hintLevel != null && (
              <span class="history-hint">
                <IconHint size="1.5em" strokeWidth={3} class="icon-hint" />
                {hintLevel}
              </span>
            )}
          </span>
        );
      })}
      {completed && (
        <span class="history-step completed-step" aria-hidden="true">
          <IconCheck size="1.5em" strokeWidth={3} class="icon-correct" /> {s.puzzle.solvedBadge}
        </span>
      )}
    </div>
  );
}

interface PuzzleViewProps {
  puzzle: Puzzle;
  dateStr: string;
  level: number;
  initialHash?: string | null;
  onNextPuzzle: () => void;
  onChanged: () => void;
  onStartTutorial?: () => void;
  autoStartTutorial?: boolean;
  onTutorialConsumed?: () => void;
}

function HintStep({ step }: { step: ExplainStep }) {
  if (step.type === "complex") {
    return (
      <div>
        {step.header}
        <ul class="hint-list">
          {step.lines.map((line, i) => (
            // oxlint-disable-next-line react/no-array-index-key
            <li key={i}>{line}</li>
          ))}
        </ul>
      </div>
    );
  }
  return <>{step.text}</>;
}

export function PuzzleView({
  puzzle,
  dateStr,
  level,
  initialHash,
  onNextPuzzle,
  onChanged,
  onStartTutorial,
  autoStartTutorial,
  onTutorialConsumed,
}: PuzzleViewProps) {
  const s = t();
  const debugMode =
    typeof window !== "undefined" &&
    (new URLSearchParams(window.location.search).has("debug") ||
      sessionStorage.getItem("debug") === "1");

  // Initialize synchronously to avoid flicker
  const initState = (() => {
    const n = puzzle.questions.length;
    const saved = initialHash ? decodeShareHash(initialHash, n) : loadState(puzzle.id, n);
    if (saved && saved.history.length > 0) {
      return saved;
    }
    const blank = puzzle.questions.map(() => ({
      marks: [...FRESH_MARKS] as Marks,
    }));
    const blankClone = cloneStates(blank);
    return {
      questions: blank,
      completed: false,
      history: [blankClone],
      historyIdx: 0,
      hints: new Map<number, number>(),
    };
  })();

  const [questions, setQuestionsRaw] = useState<QuestionState[]>(initState.questions);
  const questionsRef = useRef<QuestionState[]>(initState.questions);
  function setQuestions(qs: QuestionState[]) {
    questionsRef.current = qs;
    setQuestionsRaw(qs);
  }
  const [validity, setValidity] = useState<Validity[]>(() => {
    const fp = getFlatPuzzle(puzzle);
    const { answers, eliminated } = deriveState(
      initState.questions.map((q) => q.marks),
      puzzle.optionCount,
    );
    return answers.map((a, qi) =>
      a == null ? "neutral" : checkAnswerValidity(fp, answers, eliminated, qi),
    );
  });
  const [hintText, setHintText] = useState<ExplainStep | null>(null);
  const hintRef = useRef<{ steps: ExplainStep[]; step: number } | null>(null);
  const [debugHints, setDebugHints] = useState<ExplainStep[] | null>(null);

  const historyRef = useRef<QuestionState[][]>(initState.history);
  const historyIdxRef = useRef(initState.historyIdx);
  const [_historyVersion, setHistoryVersion] = useState(0);

  const [resetPending, setResetPending] = useState(false);
  const resetPendingRef = useRef(false);
  const [shareMenu, setShareMenu] = useState(false);
  const shareMenuRef = useRef(false);

  const [focusedQuestion, setFocusedQuestionRaw] = useState<number | null>(null);
  const [focusedOption, setFocusedOptionRaw] = useState<number | null>(null);
  const focusedQuestionRef = useRef<number | null>(null);
  const focusedOptionRef = useRef<number | null>(null);
  const gridRef = useRef<HTMLDivElement>(null);
  const nextPuzzleRef = useRef<HTMLElement>(null);
  const numberBuf = useRef({ digits: "", timer: 0 });
  const shareDropRef = useRef<HTMLButtonElement>(null);
  const controlsRef = useRef<HTMLDivElement>(null);
  const historyRef2 = useRef<HTMLDivElement>(null);

  function setFocusedQuestion(v: number | null) {
    focusedQuestionRef.current = v;
    setFocusedQuestionRaw(v);
  }
  function setFocusedOption(v: number | null) {
    focusedOptionRef.current = v;
    setFocusedOptionRaw(v);
  }

  useEffect(() => {
    if (!shareMenu) return undefined;
    const close = () => setShareMenu(false);
    document.addEventListener("click", close);
    return () => document.removeEventListener("click", close);
  }, [shareMenu]);
  const [toastMessage, setToastMessage] = useState<string | null>(null);

  useEffect(() => {
    if (!toastMessage) return undefined;
    const timer = setTimeout(() => setToastMessage(null), 2000);
    return () => clearTimeout(timer);
  }, [toastMessage]);

  function pushHistory(qs: QuestionState[]) {
    const h = historyRef.current;
    const idx = historyIdxRef.current;
    historyRef.current = h.slice(0, idx + 1);
    // Clean up hint markers for discarded future steps
    for (const key of hintMarkers.current.keys()) {
      if (key > idx) hintMarkers.current.delete(key);
    }

    const cloned = cloneStates(qs);
    if (historyRef.current.length >= 2) {
      const prev = historyRef.current[historyRef.current.length - 2];
      const last = historyRef.current[historyRef.current.length - 1];
      const lastDiff = describeDiff(prev, last);
      const newDiff = describeDiff(last, cloned);
      if (lastDiff.qi >= 0 && lastDiff.qi === newDiff.qi && lastDiff.oi === newDiff.oi) {
        const merged = describeDiff(prev, cloned);
        if (merged.qi < 0) {
          historyRef.current.pop();
          historyIdxRef.current = historyRef.current.length - 1;
        } else {
          historyRef.current[historyRef.current.length - 1] = cloned;
        }
        setHistoryVersion((v) => v + 1);
        return;
      }
    }

    historyRef.current.push(cloned);
    historyIdxRef.current = historyRef.current.length - 1;
    setHistoryVersion((v) => v + 1);
  }

  const hintMarkers = useRef<Map<number, number>>(initState.hints);

  function pushHintMarker(hintLevel: number) {
    hintMarkers.current.set(historyIdxRef.current, hintLevel);
    setHistoryVersion((v) => v + 1);
  }

  const revalidate = useCallback(
    (qs: QuestionState[]) => {
      const fp = getFlatPuzzle(puzzle);
      const { answers, eliminated } = deriveState(
        qs.map((q) => q.marks),
        puzzle.optionCount,
      );
      const result: Validity[] = answers.map((a, qi) =>
        a == null ? "neutral" : checkAnswerValidity(fp, answers, eliminated, qi),
      );
      setValidity(result);

      const isCompleted = result.every((v) => v === "valid");
      saveState(puzzle.id, {
        questions: qs,
        completed: isCompleted,
        history: historyRef.current,
        historyIdx: historyIdxRef.current,
        hints: hintMarkers.current,
      });
      if (wasStarted.current) saveMeta(puzzle.id, metaRef.current);
      onChanged();
    },
    [puzzle, onChanged],
  );

  const forceHistoryUpdate = useCallback(() => setHistoryVersion((v) => v + 1), []);

  const tutorial = useTutorial(puzzle, {
    questionsRef,
    setQuestions,
    setValidity,
    historyRef,
    historyIdxRef,
    forceHistoryUpdate,
    revalidate,
    autoStartTutorial,
    onTutorialConsumed,
    onStartTutorial,
  });

  useEffect(() => {
    if (questions.length > 0 && !tutorial.active) revalidate(questions);
  }, [questions, revalidate, tutorial.active]);

  const completed = validity.length > 0 && validity.every((v) => v === "valid");
  const completedRef = useRef(completed);
  completedRef.current = completed;
  const canUndo = historyIdxRef.current > 0;
  const canRedo = historyIdxRef.current < historyRef.current.length - 1;

  function applyChange(next: QuestionState[]) {
    if (!wasStarted.current) {
      wasStarted.current = true;
      metaRef.current.sessions = 1;
      metaRef.current.sessionStart = Date.now();
      if (initialHash) metaRef.current.fromShared = true;
      saveMeta(puzzle.id, metaRef.current);
      track("puzzle_started", {
        puzzleId: puzzle.id,
        level,
        ...getClientInfo(),
      });
    }
    pushHistory(next);
    setQuestions(next);
    setHintText(null);
    hintRef.current = null;
  }

  function handleOptionClick(questionIdx: number, optionIdx: number) {
    const next = cloneStates(questionsRef.current);
    const q = next[questionIdx];
    const current = q.marks[optionIdx];
    const hasCorrect = q.marks.indexOf("correct") >= 0;
    if (hasCorrect && current !== "correct") return;

    if (current === "unmarked") {
      q.marks[optionIdx] = "incorrect";
    } else if (current === "incorrect") {
      const existingCorrect = q.marks.indexOf("correct");
      if (existingCorrect >= 0) q.marks[existingCorrect] = "unmarked";
      q.marks[optionIdx] = "correct";
    } else {
      q.marks[optionIdx] = "unmarked";
    }

    applyChange(next);
    setResetPending(false);
    resetPendingRef.current = false;
    setFocusedQuestion(questionIdx);
    setFocusedOption(optionIdx);
  }

  function focusCurrentStep() {
    const idx = historyIdxRef.current;
    if (idx <= 0) return;
    const diff = describeDiff(historyRef.current[idx - 1], historyRef.current[idx]);
    if (diff.qi >= 0) {
      setFocusedQuestion(diff.qi);
      setFocusedOption(diff.oi);
    }
  }

  function handleUndo() {
    if (historyIdxRef.current <= 0) return;
    historyIdxRef.current--;
    setQuestions(cloneStates(historyRef.current[historyIdxRef.current]));
    setHintText(null);
    hintRef.current = null;
    setHistoryVersion((v) => v + 1);
    focusCurrentStep();
  }

  function handleRedo() {
    if (historyIdxRef.current >= historyRef.current.length - 1) return;
    historyIdxRef.current++;
    setQuestions(cloneStates(historyRef.current[historyIdxRef.current]));
    setHintText(null);
    hintRef.current = null;
    setHistoryVersion((v) => v + 1);
    focusCurrentStep();
  }

  function handleJumpTo(idx: number) {
    if (idx < 0 || idx >= historyRef.current.length) return;
    historyIdxRef.current = idx;
    setQuestions(cloneStates(historyRef.current[idx]));
    setHintText(null);
    hintRef.current = null;
    setHistoryVersion((v) => v + 1);
  }

  function handleSave() {
    if (historyRef.current.length <= 1) return;
    const prev = historyRef.current[historyIdxRef.current - 1];
    const curr = historyRef.current[historyIdxRef.current];
    if (prev && describeDiff(prev, curr).qi < 0) {
      // Current step is already a checkpoint — toggle it off (remove)
      historyRef.current.splice(historyIdxRef.current, 1);
      hintMarkers.current.delete(historyIdxRef.current);
      historyIdxRef.current--;
      setHistoryVersion((v) => v + 1);
    } else {
      pushHistory(cloneStates(questionsRef.current));
    }
  }

  function handleReset() {
    if (!resetPendingRef.current) {
      setResetPending(true);
      resetPendingRef.current = true;
      return;
    }
    setResetPending(false);
    resetPendingRef.current = false;
    const fresh = puzzle.questions.map(() => ({
      marks: [...FRESH_MARKS] as Marks,
    }));
    historyRef.current = [cloneStates(fresh)];
    historyIdxRef.current = 0;
    setQuestions(fresh);
    setHintText(null);
    hintRef.current = null;
    wasCompleted.current = false;
  }

  const solutionRef = useRef<(AnswerLetter | null)[] | null>(null);
  function getSolution(): (AnswerLetter | null)[] {
    if (!solutionRef.current) {
      const t0 = performance.now();
      solutionRef.current = solvePuzzle(getFlatPuzzle(puzzle));
      console.log(`solve: ${(performance.now() - t0).toFixed(1)}ms`);
    }
    return solutionRef.current;
  }

  function findError(answers: (AnswerLetter | null)[], eliminated: number[]): ExplainStep[] | null {
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
          {
            type: "simple",
            text: `#${qi + 1} is not ${answers[qi]} — try a different answer.`,
          },
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
    const fp = getFlatPuzzle(puzzle);
    const markSets = questionsRef.current.map((q) => q.marks);
    const { answers, eliminated } = deriveState(markSets, puzzle.optionCount);

    const errorSteps = findError(answers, eliminated);
    if (errorSteps) return { steps: errorSteps };

    const drs = deduce(fp, answers, eliminated);
    if (drs.length > 0) return { steps: explainDeduce(puzzle, fp, answers, eliminated, drs[0]) };

    const t0 = performance.now();
    const lr = lookaheadShortest(fp, answers, eliminated);
    console.log(
      `lookahead: ${(performance.now() - t0).toFixed(1)}ms, chain=${lr?.chain.length ?? "-"}`,
    );
    if (lr) {
      return { steps: explainLookahead(puzzle, fp, answers, eliminated, lr) };
    }

    return null;
  }

  // Auto-refresh all hint steps in debug mode on every move
  useEffect(() => {
    if (!debugMode || debugHints === null || completed) return;
    try {
      const result = computeHint();
      setDebugHints(result?.steps ?? null);
    } catch (e) {
      console.error("Hint error:", e);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [questions]);

  function handleHint() {
    if (!debugMode && hintRef.current && hintRef.current.step < hintRef.current.steps.length - 1) {
      hintRef.current.step++;
      setHintText(hintRef.current.steps[hintRef.current.step]);
      pushHintMarker(hintRef.current.step + 1);
      return;
    }

    let result: { steps: ExplainStep[] };
    try {
      result = computeHint() ?? {
        steps: [
          {
            type: "simple",
            text: "No obvious next step. Try making an assumption.",
          },
        ],
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

  const canShare = typeof navigator !== "undefined" && !!navigator.share;
  const hasProgress = historyRef.current.length > 1;

  async function handleSharePuzzle() {
    const url = getPuzzleUrl(dateStr, level);
    const ok = await sharePuzzleLink(url, `Refpuzzle Day #${dateStr}`);
    if (ok) setToastMessage(s.puzzle.linkCopied);
  }

  async function handleShareApp() {
    const url = window.location.origin + "/";
    const ok = await sharePuzzleLink(url, "Refpuzzle");
    if (ok) setToastMessage(s.puzzle.linkCopied);
  }

  async function handleShareProgress() {
    const url = getShareUrl(dateStr, level, {
      questions,
      completed,
      history: historyRef.current,
      historyIdx: historyIdxRef.current,
      hints: hintMarkers.current,
    });
    const ok = await sharePuzzleLink(url, `Refpuzzle Day #${dateStr}`);
    if (ok) setToastMessage(s.puzzle.linkCopied);
  }

  // Clear reset pending after timeout
  useEffect(() => {
    if (!resetPending) return undefined;
    const timer = setTimeout(() => {
      setResetPending(false);
      resetPendingRef.current = false;
    }, 3000);
    return () => clearTimeout(timer);
  }, [resetPending]);

  // Track first move
  const wasStarted = useRef(initState.history.length > 1);

  const metaRef = useRef<PuzzleMeta & { sessionStart: number | null }>({
    ...loadMeta(puzzle.id),
    sessionStart: null,
  });

  function flushElapsed() {
    const m = metaRef.current;
    if (m.sessionStart != null) {
      m.elapsedS += Math.round((Date.now() - m.sessionStart) / 1000);
      m.sessionStart = null;
      saveMeta(puzzle.id, m);
    }
  }

  // Track visibility-based sessions
  useEffect(() => {
    if (wasStarted.current) {
      metaRef.current.sessions++;
      metaRef.current.sessionStart = Date.now();
      saveMeta(puzzle.id, metaRef.current);
    }

    function onVisibility() {
      if (document.hidden) {
        flushElapsed();
      } else if (wasStarted.current && !wasCompleted.current) {
        metaRef.current.sessions++;
        metaRef.current.sessionStart = Date.now();
        saveMeta(puzzle.id, metaRef.current);
      }
    }

    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      flushElapsed();
      document.removeEventListener("visibilitychange", onVisibility);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Confetti + scroll to next puzzle on completion
  const wasCompleted = useRef(initState.completed);
  useEffect(() => {
    if (!completed || wasCompleted.current || tutorial.active) return undefined;
    wasCompleted.current = true;
    const m = metaRef.current;
    if (m.sessionStart != null) {
      m.elapsedS += Math.round((Date.now() - m.sessionStart) / 1000);
      m.sessionStart = null;
    }
    const hints = hintMarkers.current.size;
    track("puzzle_completed", {
      puzzleId: puzzle.id,
      level,
      elapsedS: m.elapsedS,
      sessions: m.sessions,
      ...(hints > 0 && { hints }),
      ...(m.fromShared && { fromShared: true }),
      ...getClientInfo(),
    });
    clearMeta(puzzle.id);
    confetti();
    const timer = setTimeout(() => {
      const btn = nextPuzzleRef.current;
      if (!btn) return;
      btn.scrollIntoView({ behavior: "smooth", block: "nearest" });
      btn.focus({ preventScroll: true });
    }, 1800);
    return () => clearTimeout(timer);
  }, [completed, level, puzzle.id, tutorial.active]);

  // Init roving tabindex on controls toolbar
  useEffect(() => {
    initRovingTabindex(controlsRef.current, "button:not(:disabled)");
  });

  // Init roving tabindex on history strip
  useEffect(() => {
    initRovingTabindex(historyRef2.current, "button.history-step:not(:disabled)");
  });

  // Scroll focused question into view
  useEffect(() => {
    if (focusedQuestion == null) return;
    const row = gridRef.current?.querySelector(`[data-qi="${focusedQuestion}"]`);
    if (row instanceof HTMLElement) row.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }, [focusedQuestion]);

  // Focus the active option button when focus state changes
  useEffect(() => {
    if (focusedQuestion == null || focusedOption == null) return;
    const btn = gridRef.current?.querySelector(
      `[data-qi="${focusedQuestion}"][data-oi="${focusedOption}"]`,
    );
    if (btn instanceof HTMLElement) btn.focus();
  }, [focusedQuestion, focusedOption]);

  const questionCount = puzzle.questions.length;

  function moveFocus(dq: number, _do: number) {
    const qi = focusedQuestionRef.current ?? 0;
    const oi = focusedOptionRef.current ?? 0;
    const nq = (qi + dq + questionCount) % questionCount;
    let no = (oi + _do + 5) % 5;
    // When moving between questions, snap to the correct option if the
    // target option is disabled (another option is marked correct)
    if (dq !== 0) {
      const marks = questionsRef.current[nq]?.marks;
      if (marks) {
        const correctIdx = marks.indexOf("correct");
        if (correctIdx >= 0) no = correctIdx;
      }
    }
    setFocusedQuestion(nq);
    setFocusedOption(no);
  }

  function navigateToQuestion(num: number) {
    if (num < 1 || num > questionCount) return;
    const qi = num - 1;
    setFocusedQuestion(qi);
    const marks = questionsRef.current[qi]?.marks;
    const correctIdx = marks?.indexOf("correct") ?? -1;
    setFocusedOption(correctIdx >= 0 ? correctIdx : (focusedOptionRef.current ?? 0));
  }

  function handleDigit(digit: number) {
    const buf = numberBuf.current;
    clearTimeout(buf.timer);
    buf.digits += String(digit);

    const parsed = parseInt(buf.digits, 10);

    if (buf.digits.length >= 2) {
      if (parsed >= 1 && parsed <= questionCount) {
        navigateToQuestion(parsed);
      } else {
        const first = parseInt(buf.digits[0], 10);
        if (first >= 1 && first <= questionCount) navigateToQuestion(first);
      }
      buf.digits = "";
      return;
    }

    // Single digit — immediately highlight if valid, even if we're still buffering
    if (parsed >= 1 && parsed <= questionCount) {
      setFocusedQuestion(parsed - 1);
      if (focusedOptionRef.current == null) setFocusedOption(0);
    }

    // Is it ambiguous? Only "1" on puzzles with 10+ questions
    if (digit * 10 <= questionCount) {
      buf.timer = window.setTimeout(() => {
        const d = parseInt(buf.digits, 10);
        if (d >= 1 && d <= questionCount) navigateToQuestion(d);
        buf.digits = "";
      }, 500);
    } else {
      if (parsed >= 1 && parsed <= questionCount) navigateToQuestion(parsed);
      buf.digits = "";
    }
  }

  // Grid keyboard navigation
  function handleGridKeyDown(e: KeyboardEvent) {
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        moveFocus(1, 0);
        break;
      case "ArrowUp":
        e.preventDefault();
        moveFocus(-1, 0);
        break;
      case "ArrowRight":
        e.preventDefault();
        moveFocus(0, 1);
        break;
      case "ArrowLeft":
        e.preventDefault();
        moveFocus(0, -1);
        break;
      case "Enter":
      case " ":
        e.preventDefault();
        if (focusedQuestionRef.current != null && focusedOptionRef.current != null && !completed) {
          handleOptionClick(focusedQuestionRef.current, focusedOptionRef.current);
        }
        break;
    }
  }

  // Share menu keyboard navigation
  function handleShareMenuKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      setShareMenu(false);
      shareMenuRef.current = false;
      shareDropRef.current?.focus();
    } else if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
    }
  }

  // Tinykeys shortcuts
  useEffect(() => {
    const g = guarded;
    const unsubscribe = tinykeys(window, {
      a: g(() => {
        if (focusedQuestionRef.current != null && !completedRef.current)
          handleOptionClick(focusedQuestionRef.current, 0);
      }),
      b: g(() => {
        if (focusedQuestionRef.current != null && !completedRef.current)
          handleOptionClick(focusedQuestionRef.current, 1);
      }),
      c: g(() => {
        if (focusedQuestionRef.current != null && !completedRef.current)
          handleOptionClick(focusedQuestionRef.current, 2);
      }),
      d: g(() => {
        if (focusedQuestionRef.current != null && !completedRef.current)
          handleOptionClick(focusedQuestionRef.current, 3);
      }),
      e: g(() => {
        if (focusedQuestionRef.current != null && !completedRef.current)
          handleOptionClick(focusedQuestionRef.current, 4);
      }),
      "0": g(() => {
        if (!completedRef.current) handleDigit(0);
      }),
      "1": g(() => {
        if (!completedRef.current) handleDigit(1);
      }),
      "2": g(() => {
        if (!completedRef.current) handleDigit(2);
      }),
      "3": g(() => {
        if (!completedRef.current) handleDigit(3);
      }),
      "4": g(() => {
        if (!completedRef.current) handleDigit(4);
      }),
      "5": g(() => {
        if (!completedRef.current) handleDigit(5);
      }),
      "6": g(() => {
        if (!completedRef.current) handleDigit(6);
      }),
      "7": g(() => {
        if (!completedRef.current) handleDigit(7);
      }),
      "8": g(() => {
        if (!completedRef.current) handleDigit(8);
      }),
      "9": g(() => {
        if (!completedRef.current) handleDigit(9);
      }),
      "$mod+z": g((ev) => {
        ev.preventDefault();
        if (!completedRef.current) handleUndo();
      }),
      "$mod+Shift+z": g((ev) => {
        ev.preventDefault();
        if (!completedRef.current) handleRedo();
      }),
      "$mod+y": g((ev) => {
        ev.preventDefault();
        if (!completedRef.current) handleRedo();
      }),
      h: g(() => {
        if (!completedRef.current) handleHint();
      }),
      p: g(() => {
        if (!completedRef.current) handleSave();
      }),
      j: g(() => {
        if (!completedRef.current) moveFocus(1, 0);
      }),
      k: g(() => {
        if (!completedRef.current) moveFocus(-1, 0);
      }),
    });
    return unsubscribe;
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <>
      {(tutorial.active || tutorial.welcome) && (
        <div
          class={`tutorial-scrim${tutorial.welcome && !tutorial.active ? " tutorial-scrim-welcome" : ""}`}
        />
      )}
      <div class={`puzzle-view${tutorial.active || tutorial.welcome ? " tutorial-active" : ""}`}>
        {tutorial.active && <div class="tutorial-heading">Tutorial</div>}
        {tutorial.welcome && !tutorial.active && (
          <TutorialWelcome
            onStart={tutorial.startFromWelcome}
            onDismiss={tutorial.dismissWelcome}
          />
        )}
        {/* Questions */}
        <TutorialHighlightCtx.Provider value={tutorial.active ? tutorial.highlight : null}>
          <div
            ref={gridRef}
            class={`questions-grid${puzzle.questions.length <= 3 ? " single-col" : ""}${tutorial.active ? " tutorial-dimmed" : ""}`}
            style={{
              gridTemplateRows: `repeat(${Math.ceil(puzzle.questions.length / 2) * 2}, auto)`,
            }}
            onKeyDown={handleGridKeyDown}
            onFocusCapture={() => {
              if (focusedQuestionRef.current == null) {
                setFocusedQuestion(0);
                setFocusedOption(0);
              }
            }}
          >
            {puzzle.questions.map((qDef, qi) => (
              <QuestionRow
                key={qDef.questionType.type + JSON.stringify(qDef.questionType)}
                index={qi}
                question={qDef}
                marks={questions[qi]?.marks ?? FRESH_MARKS}
                validity={validity[qi] ?? "neutral"}
                disabled={completed || tutorial.active}
                focusedOption={focusedQuestion === qi ? focusedOption : null}
                defaultFocus={focusedQuestion == null && qi === 0}
                onOptionClick={(oi) => handleOptionClick(qi, oi)}
              />
            ))}
          </div>
        </TutorialHighlightCtx.Provider>

        {tutorial.active && (
          <TutorialOverlay
            steps={tutorial.steps}
            onDismiss={tutorial.dismiss}
            onSetHighlight={tutorial.setHighlight}
            onApplyStep={tutorial.applyStep}
            onUnapplyStep={tutorial.unapplyStep}
            onDone={() => tutorial.setHighlight(null)}
          />
        )}

        {/* Hint display */}
        {!completed && debugMode && debugHints && (
          <div class="puzzle-hint">
            <ol>
              {debugHints.map((step, i) => (
                // oxlint-disable-next-line react/no-array-index-key
                <li key={i}>
                  <HintStep step={step} />
                </li>
              ))}
            </ol>
          </div>
        )}
        {!completed && !debugMode && hintText && (
          <div class="puzzle-hint">
            <HintStep step={hintText} />
            {hintRef.current && hintRef.current.step < hintRef.current.steps.length - 1 && (
              <button class="hint-more" onClick={handleHint}>
                {s.puzzle.more}
              </button>
            )}
          </div>
        )}

        {/* Completion banner */}
        {completed && !tutorial.active && (
          <div class="puzzle-complete">
            <span>{s.puzzle.solved}</span>
            {level < 5 ? (
              <button
                // oxlint-disable-next-line typescript/no-unsafe-type-assertion
                ref={nextPuzzleRef as Ref<HTMLButtonElement>}
                class="next-puzzle-btn"
                onClick={onNextPuzzle}
              >
                {s.puzzle.nextPuzzle} &rarr;
              </button>
            ) : (
              <a
                // oxlint-disable-next-line typescript/no-unsafe-type-assertion
                ref={nextPuzzleRef as Ref<HTMLAnchorElement>}
                href="/past"
                class="next-puzzle-btn"
              >
                {s.daily.pastPuzzles} &rarr;
              </a>
            )}
          </div>
        )}

        {/* Controls */}
        <div
          ref={controlsRef}
          class="puzzle-controls"
          role="toolbar"
          onKeyDown={arrowNavHandler("button:not(:disabled)")}
        >
          <button
            class="toolbar-icon-btn"
            onClick={handleUndo}
            disabled={completed || !canUndo}
            title={s.puzzle.undo}
          >
            <IconUndo />
          </button>
          <button
            class="toolbar-icon-btn"
            onClick={handleRedo}
            disabled={completed || !canRedo}
            title={s.puzzle.redo}
          >
            <IconRedo />
          </button>
          <button class="toolbar-accent-btn" onClick={handleSave} disabled={completed}>
            <IconPin size="0.9em" /> {s.puzzle.checkpoint}
          </button>
          <button
            class="toolbar-accent-btn"
            onClick={handleHint}
            onMouseEnter={getSolution}
            onFocus={getSolution}
            onTouchStart={getSolution}
            disabled={completed}
            title={s.puzzle.hint}
          >
            <IconHint size="0.9em" class="icon-hint" /> {s.puzzle.hint}
          </button>
          {tutorial.isIntro && (
            <button
              class={`toolbar-accent-btn${tutorial.done ? " tutorial-highlight-btn" : ""}`}
              onClick={tutorial.startManual}
              disabled={completed || tutorial.active}
            >
              Tutorial
            </button>
          )}
          <span class="controls-spacer"></span>
          <span class="split-btn">
            <button class="toolbar-accent-btn" onClick={handleSharePuzzle}>
              <IconShare size="0.9em" /> {canShare ? s.puzzle.share : s.puzzle.copyLink}
            </button>
            <span class="split-btn-wrapper">
              <button
                ref={shareDropRef}
                class="toolbar-accent-btn split-btn-drop"
                aria-haspopup="true"
                aria-expanded={shareMenu}
                onClick={(e) => {
                  e.stopPropagation();
                  setShareMenu((v) => {
                    shareMenuRef.current = !v;
                    return !v;
                  });
                }}
                onKeyDown={(e) => {
                  if (e.key === "ArrowDown" || e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    e.stopPropagation();
                    setShareMenu(true);
                    shareMenuRef.current = true;
                    requestAnimationFrame(() => {
                      const item = shareDropRef.current
                        ?.closest(".split-btn-wrapper")
                        ?.querySelector(".split-btn-menu button");
                      if (item instanceof HTMLElement) item.focus();
                    });
                  } else if (e.key === "Escape" && shareMenuRef.current) {
                    e.preventDefault();
                    e.stopPropagation();
                    setShareMenu(false);
                    shareMenuRef.current = false;
                  }
                }}
              >
                <IconChevronDown size="1em" />
              </button>
              {shareMenu && (
                <div class="split-btn-menu" onKeyDown={handleShareMenuKeyDown}>
                  <button
                    role="menuitem"
                    onClick={() => {
                      setShareMenu(false);
                      shareMenuRef.current = false;
                      handleShareApp();
                    }}
                  >
                    {canShare ? s.puzzle.shareApp : s.puzzle.copyApp}
                  </button>
                  {hasProgress && (
                    <button
                      role="menuitem"
                      onClick={() => {
                        setShareMenu(false);
                        shareMenuRef.current = false;
                        handleShareProgress();
                      }}
                    >
                      {canShare ? s.puzzle.shareWithProgress : s.puzzle.copyWithProgress}
                    </button>
                  )}
                </div>
              )}
              {toastMessage && <span class="share-toast">{toastMessage}</span>}
            </span>
          </span>
          <button
            class={`toolbar-accent-btn ${resetPending ? "reset-confirm" : ""}`}
            onClick={handleReset}
            disabled={historyRef.current.length <= 1}
          >
            <IconReset size="0.9em" />
            <span>{s.puzzle.reset}</span>
            {resetPending && <span class="reset-overlay">{s.puzzle.resetConfirm}</span>}
          </button>
        </div>

        {historyRef.current.length > 1 && (
          <HistoryStrip
            history={historyRef.current}
            currentIdx={historyIdxRef.current}
            hints={hintMarkers.current}
            completed={completed}
            onJump={handleJumpTo}
            containerRef={historyRef2}
          />
        )}
      </div>
    </>
  );
}
