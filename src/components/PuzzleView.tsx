import { useState, useEffect, useCallback, useRef } from "preact/hooks";
import { tinykeys } from "tinykeys";
import type { Marks, Puzzle } from "../engine/types.ts";
import { FRESH_MARKS } from "../engine/types.ts";
import { deriveState, isValid, V_NEUTRAL } from "../engine/state.ts";
import type { Validity } from "../engine/state.ts";
import { findMistake } from "../engine/mistake.ts";
import { wasmReady, createPuzzleHandle, type PuzzleHandle } from "../lib/wasm.ts";
import { loadState, saveState, saveMeta, cloneStates } from "../lib/store.ts";
import type { QuestionState } from "../lib/store.ts";
import { decodeShareHash, getShareUrl, getPuzzleUrl } from "../lib/share.ts";
import { guarded, arrowNavHandler, initRovingTabindex } from "../lib/keyboard.ts";
import { confetti } from "../lib/confetti.ts";
import { track, getClientInfo } from "../lib/analytics.ts";
import { t } from "../i18n/index.ts";
import { QuestionRow } from "./QuestionRow.tsx";
import { HistoryStrip, describeDiff, lastCheckpointIdx } from "./HistoryStrip.tsx";
import { HintStep } from "./HintStep.tsx";
import { CoachText } from "./CoachText.tsx";
import { CoachArrows } from "./CoachArrows.tsx";
import { useL1Coach } from "./useL1Coach.ts";
import { useForceUpdate } from "../lib/hooks.ts";
import { useAnalytics } from "./useAnalytics.ts";
import { useHintEngine } from "./useHintEngine.ts";
import { ShareSheet } from "./ShareSheet.tsx";
import {
  IconUndo,
  IconRedo,
  IconPin,
  IconHint,
  IconChevronDown,
  IconReset,
  IconShare,
} from "./Icons.tsx";
import type { Ref } from "preact";

interface PuzzleViewProps {
  puzzle: Puzzle;
  dateStr: string;
  level: number;
  initialHash?: string | null;
  /** Playground mode: render from the URL only, never touch localStorage. */
  ephemeral?: boolean;
  onNextPuzzle: () => void;
  onChanged: () => void;
}

export function PuzzleView({
  puzzle,
  dateStr,
  level,
  initialHash,
  ephemeral,
  onNextPuzzle,
  onChanged,
}: PuzzleViewProps) {
  const s = t();
  const debugMode =
    typeof window !== "undefined" &&
    (new URLSearchParams(window.location.search).has("debug") ||
      sessionStorage.getItem("debug") === "1");

  // Ephemeral (playground) mode persists nothing: the puzzle is fully described
  // by the URL. Gating the two saveState calls is sufficient — saveMeta /
  // loadMeta all no-op without an existing entry, and loadState returns null,
  // so no other store touchpoint can write.

  // Initialize synchronously to avoid flicker
  const initState = (() => {
    const n = puzzle.questions.length;
    const saved = initialHash
      ? decodeShareHash(initialHash, n)
      : ephemeral
        ? null
        : loadState(puzzle.id, n);
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
      stale: false,
      history: [blankClone],
      historyIdx: 0,
      hints: new Map<number, number>(),
      fails: new Map<number, number>(),
    };
  })();

  const analytics = useAnalytics(puzzle.id, {
    level,
    initialHash,
    initStarted: initState.history.length > 1,
    initCompleted: initState.completed,
  });

  const [questions, setQuestionsRaw] = useState<QuestionState[]>(initState.questions);
  const questionsRef = useRef<QuestionState[]>(initState.questions);
  function setQuestions(qs: QuestionState[]) {
    questionsRef.current = qs;
    setQuestionsRaw(qs);
  }
  const [validity, setValidity] = useState<Validity[]>(() =>
    new Array(initState.questions.length).fill(V_NEUTRAL),
  );
  const handleRef = useRef<PuzzleHandle | null>(null);
  const [handleReady, setHandleReady] = useState(false);
  useEffect(() => {
    let canceled = false;
    void (async () => {
      await wasmReady();
      if (canceled) return;
      const handle = createPuzzleHandle(puzzle.compact, puzzle.id);
      handleRef.current = handle;
      const initial = handle.checkAllAnswers(
        questionsRef.current.map((q) => q.marks),
        puzzle.optionCount,
      );
      setValidity(initial);
      setHandleReady(true);
    })();
    return () => {
      canceled = true;
      handleRef.current?.free();
      handleRef.current = null;
      setHandleReady(false);
    };
  }, [puzzle]);
  const historyRef = useRef<QuestionState[][]>(initState.history);
  const historyIdxRef = useRef(initState.historyIdx);
  const forceHistoryUpdate = useForceUpdate();

  const initCompleted = initState.completed || validity.every(isValid);
  const tabStateRef = useRef({
    started: initState.history.length > 1,
    completed: initCompleted,
    stale: initState.stale,
  });
  useEffect(() => {
    if (initCompleted && !initState.completed && !ephemeral) {
      saveState(puzzle.id, {
        questions: initState.questions,
        completed: true,
        stale: false,
        history: initState.history,
        historyIdx: initState.historyIdx,
        hints: initState.hints,
        fails: initState.fails,
      });
      onChanged();
    }
  }, [initCompleted, initState, puzzle.id, onChanged, ephemeral]);
  const historyBurstRef = useRef({ lastTime: 0 });

  function trackHistoryBurst() {
    const now = Date.now();
    if (now - historyBurstRef.current.lastTime > 15_000) {
      analytics.meta.current.historyBursts++;
      saveMeta(puzzle.id, analytics.meta.current);
    }
    historyBurstRef.current.lastTime = now;
  }

  const [resetPending, setResetPending] = useState(false);
  const resetPendingRef = useRef(false);
  /** The Checkpoint button's verdict. Shares the hint's slot; only one speaks. */
  const [checkpointNote, setCheckpointNote] = useState<string | null>(null);
  const [shareSheet, setShareSheet] = useState<{ url: string; title: string } | null>(null);
  const [shareMenu, setShareMenu] = useState(false);
  const shareMenuRef = useRef(false);
  const shareDropRef = useRef<HTMLButtonElement>(null);

  const [focusedQuestion, setFocusedQuestionRaw] = useState<number | null>(null);
  const [focusedOption, setFocusedOptionRaw] = useState<number | null>(null);
  const focusedQuestionRef = useRef<number | null>(null);
  const focusedOptionRef = useRef<number | null>(null);
  const gridRef = useRef<HTMLDivElement>(null);
  const nextPuzzleRef = useRef<HTMLElement>(null);
  const puzzleCompleteRef = useRef<HTMLDivElement>(null);
  const numberBuf = useRef({ digits: "", timer: 0 });
  const controlsRef = useRef<HTMLDivElement>(null);
  const historyStripRef = useRef<HTMLDivElement>(null);

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

  /**
   * Markers on steps a history rewrite discards fold onto the branch point —
   * the step just before the newly added one — rather than being deleted, so
   * no rewind can erase the record. Refusal counts add up; hint markers keep
   * the deepest level reached, since that's what the value means (and the wire
   * format caps at `h4`).
   */
  function foldMarkers(branchIdx: number) {
    for (const [key, hintLevel] of [...hintMarkers.current]) {
      if (key <= branchIdx) continue;
      hintMarkers.current.delete(key);
      hintMarkers.current.set(
        branchIdx,
        Math.max(hintMarkers.current.get(branchIdx) ?? 0, hintLevel),
      );
    }
    for (const [key, count] of [...failMarkers.current]) {
      if (key <= branchIdx) continue;
      failMarkers.current.delete(key);
      failMarkers.current.set(branchIdx, (failMarkers.current.get(branchIdx) ?? 0) + count);
    }
  }

  function pushHistory(qs: QuestionState[]) {
    const h = historyRef.current;
    const idx = historyIdxRef.current;
    historyRef.current = h.slice(0, idx + 1);
    foldMarkers(idx);

    const cloned = cloneStates(qs);
    if (historyRef.current.length >= 2) {
      const prev = historyRef.current[historyRef.current.length - 2];
      const last = historyRef.current[historyRef.current.length - 1];
      const lastDiff = describeDiff(prev, last);
      const newDiff = describeDiff(last, cloned);
      if (lastDiff.qi >= 0 && lastDiff.qi === newDiff.qi && lastDiff.oi === newDiff.oi) {
        const merged = describeDiff(prev, cloned);
        if (merged.qi < 0) {
          // The step un-did itself out of existence; its markers slide back too.
          historyRef.current.pop();
          historyIdxRef.current = historyRef.current.length - 1;
          foldMarkers(historyIdxRef.current);
        } else {
          historyRef.current[historyRef.current.length - 1] = cloned;
        }
        forceHistoryUpdate();
        return;
      }
    }

    historyRef.current.push(cloned);
    historyIdxRef.current = historyRef.current.length - 1;
    forceHistoryUpdate();
  }

  const hintMarkers = useRef<Map<number, number>>(initState.hints);
  /**
   * Refused checkpoint presses, keyed by the step the press landed on. History
   * rewrites can't shake them off — `foldMarkers` slides them onto the branch
   * point instead of deleting them. `meta.checkpointFails` tallies the same
   * presses monotonically for scoring.
   */
  const failMarkers = useRef<Map<number, number>>(initState.fails);

  function pushFailMarker() {
    const map = failMarkers.current;
    const idx = historyIdxRef.current;
    map.set(idx, (map.get(idx) ?? 0) + 1);
    analytics.meta.current.checkpointFails++;
    saveMeta(puzzle.id, analytics.meta.current);
    forceHistoryUpdate();
  }

  function pushHintMarker(hintLevel: number) {
    const map = hintMarkers.current;
    const idx = historyIdxRef.current;
    // Deepest level reached at this step — a fresh press never lowers a level
    // that folded here from rewritten history.
    map.set(idx, Math.max(map.get(idx) ?? 0, hintLevel));
    analytics.meta.current.hints++;
    saveMeta(puzzle.id, analytics.meta.current);
    forceHistoryUpdate();
  }

  const revalidate = useCallback(
    (qs: QuestionState[]) => {
      const handle = handleRef.current;
      const result: Validity[] = handle
        ? handle.checkAllAnswers(
            qs.map((q) => q.marks),
            puzzle.optionCount,
          )
        : new Array(qs.length).fill(V_NEUTRAL);
      setValidity(result);

      const isCompleted = result.every(isValid);
      // The board just faced the current puzzle version, so a solve retires the
      // stale flag; short of one, the flag stands as the last sweep left it.
      const nowStale = tabStateRef.current.stale && !isCompleted;
      if (!ephemeral) {
        saveState(puzzle.id, {
          questions: qs,
          completed: isCompleted,
          stale: nowStale,
          history: historyRef.current,
          historyIdx: historyIdxRef.current,
          hints: hintMarkers.current,
          fails: failMarkers.current,
        });
      }
      if (analytics.wasStarted.current && !analytics.wasCompleted.current)
        saveMeta(puzzle.id, analytics.meta.current);
      const nowStarted = historyRef.current.length > 1;
      if (
        nowStarted !== tabStateRef.current.started ||
        isCompleted !== tabStateRef.current.completed ||
        nowStale !== tabStateRef.current.stale
      ) {
        tabStateRef.current = { started: nowStarted, completed: isCompleted, stale: nowStale };
        onChanged();
      }
    },
    [puzzle, onChanged, analytics.meta, analytics.wasStarted, analytics.wasCompleted, ephemeral],
  );

  const completed = validity.length > 0 && validity.every(isValid);
  const completedRef = useRef(completed);
  completedRef.current = completed;

  // L1-only ambient coach: calm intro text, idle nudges, and mistake notes,
  // with reference arrows. Silent the instant the player engages. Replaces the
  // old auto-solve tutorial; higher levels and playground get nothing.
  const coachEnabled = level === 1 && !ephemeral;
  const coachTextRef = useRef<HTMLDivElement>(null);
  const coach = useL1Coach(puzzle, {
    enabled: coachEnabled,
    handleRef,
    handleReady,
    questions,
    started: historyRef.current.length > 1,
    completed,
    // Coach hints land on the history track + hint count, like the Hint button.
    onHint: pushHintMarker,
  });
  const canUndo = historyIdxRef.current > 0;
  const canRedo = historyIdxRef.current < historyRef.current.length - 1;

  const hints = useHintEngine(puzzle, {
    questionsRef,
    debugMode,
    pushHintMarker,
    completed,
    questions,
    handleRef,
  });

  // Whether this session made any local change. v1 share URLs carry no
  // completed flag — completion is derived once wasm loads — so this is what
  // separates "you solved it" from "it arrived solved".
  const interactedRef = useRef(false);

  function applyChange(next: QuestionState[]) {
    interactedRef.current = true;
    analytics.markStarted();
    pushHistory(next);
    setQuestions(next);
    revalidate(next);
    hints.clear();
    setCheckpointNote(null);
  }

  /**
   * The board the last checkpoint at or behind the cursor verified, or null.
   * Every mark on it is known correct, so those cells are locked; rewinding
   * past the pin unlocks them (and marking there discards it).
   */
  function checkpointBoard(): QuestionState[] | null {
    const cpIdx = lastCheckpointIdx(historyRef.current, historyIdxRef.current);
    return cpIdx > 0 ? historyRef.current[cpIdx] : null;
  }

  /**
   * Cells playing the checkpointed sweep (see `.option-btn.sweep`), as a
   * per-question option bitmask: what a landing checkpoint just settled, or the
   * one cell a click bounced off. Cleared once the animation has run.
   */
  const [sweepMasks, setSweepMasks] = useState<number[] | null>(null);
  const sweepTimer = useRef(0);
  function playSweep(masks: number[]) {
    clearTimeout(sweepTimer.current);
    setSweepMasks(masks);
    sweepTimer.current = window.setTimeout(() => setSweepMasks(null), 600);
  }
  useEffect(() => () => clearTimeout(sweepTimer.current), []);

  /**
   * What the checkpoint at the cursor settled that the one before it hadn't.
   * A question with an answer contributes only that answer — its eliminations
   * are subsumed by it, so re-announcing them would be noise.
   */
  function newlySettledMasks(): number[] {
    const cpIdx = lastCheckpointIdx(historyRef.current, historyIdxRef.current);
    const prevIdx = lastCheckpointIdx(historyRef.current, cpIdx - 1);
    const prev = prevIdx > 0 ? historyRef.current[prevIdx] : null;
    return historyRef.current[cpIdx].map((q, qi) => {
      const isNew = (oi: number) => prev == null || prev[qi].marks[oi] === "unmarked";
      const answerOi = q.marks.indexOf("correct");
      if (answerOi >= 0) return isNew(answerOi) ? 1 << answerOi : 0;
      let mask = 0;
      for (let oi = 0; oi < puzzle.optionCount; oi++) {
        if (q.marks[oi] === "incorrect" && isNew(oi)) mask |= 1 << oi;
      }
      return mask;
    });
  }

  function handleOptionClick(questionIdx: number, optionIdx: number) {
    const verified = checkpointBoard();
    if (verified && verified[questionIdx].marks[optionIdx] !== "unmarked") {
      playSweep(puzzle.questions.map((_q, qi) => (qi === questionIdx ? 1 << optionIdx : 0)));
      return;
    }
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

  const optionClickRef = useRef(handleOptionClick);
  optionClickRef.current = handleOptionClick;
  const stableOptionClick = useCallback(
    (qi: number, oi: number) => optionClickRef.current(qi, oi),
    [],
  );

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
    trackHistoryBurst();
    historyIdxRef.current--;
    const qs = cloneStates(historyRef.current[historyIdxRef.current]);
    setQuestions(qs);
    revalidate(qs);
    hints.clear();
    setCheckpointNote(null);
    forceHistoryUpdate();
    focusCurrentStep();
  }

  function handleRedo() {
    if (historyIdxRef.current >= historyRef.current.length - 1) return;
    trackHistoryBurst();
    historyIdxRef.current++;
    const qs = cloneStates(historyRef.current[historyIdxRef.current]);
    setQuestions(qs);
    revalidate(qs);
    hints.clear();
    setCheckpointNote(null);
    forceHistoryUpdate();
    focusCurrentStep();
  }

  function handleJumpTo(idx: number) {
    if (idx < 0 || idx >= historyRef.current.length) return;
    trackHistoryBurst();
    historyIdxRef.current = idx;
    const qs = cloneStates(historyRef.current[idx]);
    setQuestions(qs);
    revalidate(qs);
    hints.clear();
    setCheckpointNote(null);
    forceHistoryUpdate();
  }

  /**
   * Grant a checkpoint if nothing on the board contradicts the key. Marks the
   * last checkpoint verified are locked against editing, so a re-verified
   * range can only have grown — the one way to fail is a wrong new mark, which
   * lands a refusal marker and opens the hint panel's escalation ladder.
   */
  function handleSave() {
    const idx = historyIdxRef.current;
    if (idx < 1 || lastCheckpointIdx(historyRef.current, idx) === idx) return;
    const current = questionsRef.current;
    const marks = current.map((q) => q.marks);

    const { answers, eliminated } = deriveState(marks, puzzle.optionCount);
    const mistake = findMistake(answers, eliminated, hints.getSolution());
    if (mistake) {
      setCheckpointNote(null);
      pushFailMarker();
      hints.showMistake(mistake, s.puzzle.checkpointWrong);
      return;
    }

    analytics.meta.current.checkpoints++;
    saveMeta(puzzle.id, analytics.meta.current);
    pushHistory(cloneStates(current));
    // The pin is only in the history ref until something else persists — commit
    // it now so a granted checkpoint survives a reload.
    revalidate(current);
    hints.clear();
    setCheckpointNote(s.puzzle.checkpointSet);
    // Announce what this checkpoint settled that the last one hadn't.
    playSweep(newlySettledMasks());
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
    // Explicit, or the next mark's foldMarkers would pile them all onto Start.
    hintMarkers.current = new Map();
    failMarkers.current = new Map();
    setQuestions(fresh);
    revalidate(fresh);
    hints.clear();
    setCheckpointNote(null);
    analytics.wasCompleted.current = false;
  }

  // Per-question bitmask of the cells the last checkpoint verified.
  const verifiedBoard = checkpointBoard();
  const checkpointedMasks = puzzle.questions.map((_q, qi) => {
    if (!verifiedBoard) return 0;
    let mask = 0;
    for (let oi = 0; oi < puzzle.optionCount; oi++) {
      if (verifiedBoard[qi].marks[oi] !== "unmarked") mask |= 1 << oi;
    }
    return mask;
  });

  const hasProgress = historyRef.current.length > 1;
  // Nothing to verify on a blank board, and a checkpoint can't checkpoint itself.
  const canCheckpoint =
    historyIdxRef.current > 0 &&
    describeDiff(
      historyRef.current[historyIdxRef.current - 1],
      historyRef.current[historyIdxRef.current],
    ).qi >= 0;

  function openSharePuzzle() {
    setShareSheet({ url: getPuzzleUrl(dateStr, level), title: s.puzzle.share });
  }
  function openShareApp() {
    setShareSheet({ url: `${window.location.origin}/`, title: s.puzzle.shareApp });
  }
  function openShareProgress() {
    setShareSheet({
      url: getShareUrl(dateStr, level, {
        questions,
        completed,
        stale: false,
        history: historyRef.current,
        historyIdx: historyIdxRef.current,
        hints: hintMarkers.current,
        fails: failMarkers.current,
      }),
      title: s.puzzle.shareWithProgress,
    });
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

  // Confetti + scroll to next puzzle on completion
  useEffect(() => {
    if (!completed || analytics.wasCompleted.current) return undefined;
    analytics.wasCompleted.current = true;
    // Completion that predates any local change arrived via a share URL or
    // storage — someone else's solve. Acknowledge it, celebrate nothing.
    if (!interactedRef.current) return undefined;
    const m = analytics.meta.current;
    if (m.sessionStart != null) {
      m.elapsedS += Math.round((Date.now() - m.sessionStart) / 1000);
      m.sessionStart = null;
    }
    track("puzzle_completed", {
      puzzleId: puzzle.id,
      level,
      elapsedS: m.elapsedS,
      sessions: m.sessions,
      // Zeroes drop to undefined.
      hints: m.hints || undefined,
      checkpoints: m.checkpoints || undefined,
      checkpointFails: m.checkpointFails || undefined,
      historyBursts: m.historyBursts || undefined,
      fromShared: m.fromShared || undefined,
      ...getClientInfo(),
    });
    // No clearMeta: saveState already swapped the ledger to the outcome
    // family when the completing mark was saved; the event above reads the
    // in-memory copy.
    confetti();
    // Scroll the whole completion banner into view right as the confetti starts — the
    // celebration overlay masks the viewport motion, so it reads smoother than scrolling after.
    puzzleCompleteRef.current?.scrollIntoView({ behavior: "smooth", block: "nearest" });
    nextPuzzleRef.current?.focus({ preventScroll: true });
    return undefined;
  }, [completed, level, puzzle.id, analytics.meta, analytics.wasCompleted]);

  // Init roving tabindex on controls toolbar
  useEffect(() => {
    initRovingTabindex(controlsRef.current, "button:not(:disabled)");
  });

  // Init roving tabindex on history strip
  useEffect(() => {
    initRovingTabindex(historyStripRef.current, "button.history-step:not(:disabled)");
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
        if (!completedRef.current) hints.handleHint();
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
      <div class="puzzle-view">
        {coachEnabled && !completed && <CoachText message={coach.message} boxRef={coachTextRef} />}
        {/* Questions */}
        <div
          ref={gridRef}
          class={`questions-grid${puzzle.questions.length <= 3 ? " single-col" : ""}`}
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
              key={qDef.text}
              index={qi}
              question={qDef}
              marks={questions[qi]?.marks ?? FRESH_MARKS}
              validity={validity[qi] ?? "neutral"}
              disabled={completed}
              checkpointedMask={checkpointedMasks[qi]}
              sweepMask={sweepMasks?.[qi] ?? 0}
              focusedOption={focusedQuestion === qi ? focusedOption : null}
              defaultFocus={focusedQuestion == null && qi === 0}
              onOptionClick={stableOptionClick}
            />
          ))}
        </div>

        {coachEnabled && !completed && (
          <CoachArrows
            message={coach.message}
            gridRef={gridRef}
            textRef={coachTextRef}
            marks={questions.map((q) => q.marks)}
            optionCount={puzzle.optionCount}
          />
        )}

        {/* Hint display */}
        {!completed && debugMode && hints.debugHints && (
          <div class="puzzle-hint">
            <ol>
              {hints.debugHints.map((step, i) => (
                // oxlint-disable-next-line react/no-array-index-key
                <li key={i}>
                  <HintStep step={step} />
                </li>
              ))}
            </ol>
          </div>
        )}
        {!completed && !debugMode && hints.hintText && (
          <div class="puzzle-hint">
            <HintStep step={hints.hintText} />
            {hints.hasMore && (
              <button class="hint-more" onClick={hints.handleHint}>
                {s.puzzle.more}
              </button>
            )}
          </div>
        )}

        {/* Checkpoint verdict */}
        {!completed && checkpointNote && <div class="puzzle-note">{checkpointNote}</div>}

        {/* Completion banner */}
        {completed && (
          <div ref={puzzleCompleteRef} class="puzzle-complete">
            <span>{s.puzzle.solved}</span>
            {level < 6 ? (
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
                href="/archive"
                class="next-puzzle-btn"
              >
                {s.daily.archive} &rarr;
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
          <button
            class="toolbar-accent-btn"
            onClick={handleSave}
            disabled={completed || !canCheckpoint}
          >
            <IconPin size="0.9em" /> {s.puzzle.checkpoint}
          </button>
          <button
            class="toolbar-accent-btn"
            onClick={hints.handleHint}
            onMouseEnter={hints.getSolution}
            onFocus={hints.getSolution}
            onTouchStart={hints.getSolution}
            disabled={completed}
            title={s.puzzle.hint}
          >
            <IconHint size="0.9em" class="icon-hint" /> {s.puzzle.hint}
          </button>
          <span class="controls-spacer"></span>
          <span class="split-btn">
            <button class="toolbar-accent-btn" onClick={openSharePuzzle}>
              <IconShare size="0.9em" /> {s.puzzle.share}
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
                      openShareApp();
                    }}
                  >
                    {s.puzzle.shareApp}
                  </button>
                  {hasProgress && (
                    <button
                      role="menuitem"
                      onClick={() => {
                        setShareMenu(false);
                        shareMenuRef.current = false;
                        openShareProgress();
                      }}
                    >
                      {s.puzzle.shareWithProgress}
                    </button>
                  )}
                </div>
              )}
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
            fails={failMarkers.current}
            completed={completed}
            onJump={handleJumpTo}
            containerRef={historyStripRef}
          />
        )}
      </div>
      {shareSheet && (
        <ShareSheet
          url={shareSheet.url}
          title={shareSheet.title}
          onClose={() => setShareSheet(null)}
        />
      )}
    </>
  );
}
