import { useState, useEffect, useLayoutEffect, useCallback, useMemo, useRef } from "preact/hooks";
import { tinykeys } from "tinykeys";
import type { Puzzle } from "../engine/types.ts";
import { FRESH_MARKS, LETTERS } from "../engine/types.ts";
import { deriveState, isValid, V_NEUTRAL } from "../engine/state.ts";
import type { Validity } from "../engine/state.ts";
import { findMistake } from "../engine/mistake.ts";
import { wasmReady, createPuzzleHandle, type PuzzleHandle } from "../lib/wasm.ts";
import { loadState, saveState, saveMeta, loadMeta, cloneStates } from "../lib/store.ts";
import type { FailMarker, HintMarker, QuestionState } from "../lib/store.ts";
import { decodeShareHash, getPuzzleUrl } from "../lib/share.ts";
import { guarded, arrowNavHandler, initRovingTabindex } from "../lib/keyboard.ts";
import { classNames } from "../lib/classNames.ts";
import { debugEnabled } from "../lib/debug.ts";
import { track, getClientInfo } from "../lib/analytics.ts";
import { t } from "../i18n/index.ts";
import { QuestionRow } from "./QuestionRow.tsx";
import { HistoryStrip, describeDiff, lastCheckpointIdx } from "./HistoryStrip.tsx";
import { questionOutcomes, storedSolveStats } from "../lib/solve-summary.ts";
import { HintStep } from "./HintStep.tsx";
import { CoachText } from "./CoachText.tsx";
import { CoachArrows } from "./CoachArrows.tsx";
import { NudgeCallout } from "./NudgeCallout.tsx";
import { useL1Coach } from "./useL1Coach.ts";
import { useForceUpdate, useVisibleTimeout } from "../lib/hooks.ts";
import { useAnalytics } from "./useAnalytics.ts";
import { useHintEngine } from "./useHintEngine.ts";
import { PuzzleShareDialog, type ShareMode } from "./PuzzleShareDialog.tsx";
import { SolvedDialog } from "./SolvedDialog.tsx";
import { useIdleNudge } from "./useIdleNudge.ts";
import { IconUndo, IconRedo, IconPin, IconHint } from "./Icons.tsx";
import { LEVELS } from "../puzzles/daily.ts";

/** The mark shortcuts, one per option letter. */
const OPTION_KEYS = LETTERS.map((letter) => letter.toLowerCase());

/** How long `.option-btn.sweep` stays on: the CSS duration plus slack. */
const SWEEP_MS = 1000;

/** How long a granted checkpoint's note stays up while the tab is visible. */
const NOTE_MS = 25_000;

/**
 * Geometry for `.option-btn.sweep`: the board's span, and each sweeping
 * cell's diagonal distance (x + y) past the first sweeping cell. Board-sized
 * so the wave's speed doesn't depend on how many cells light; anchored on the
 * cells so a lone refused click is hit at once.
 */
function placeSweep(grid: HTMLElement) {
  const cells = Array.from(grid.querySelectorAll<HTMLElement>(".option-btn.sweep"));
  const diagonals = cells.map((cell) => {
    const { left, top } = cell.getBoundingClientRect();
    return left + top;
  });
  const first = Math.min(...diagonals);
  const { width, height } = grid.getBoundingClientRect();
  grid.style.setProperty("--sweep-span", `${width + height}px`);
  cells.forEach((cell, i) => cell.style.setProperty("--sweep-d", `${diagonals[i] - first}px`));
}

/** A blank board: every question with every option unmarked. */
function freshBoard(puzzle: Puzzle): QuestionState[] {
  return puzzle.questions.map(() => ({ marks: [...FRESH_MARKS] }));
}

/**
 * Where this mount picks up: a shared board from the URL, the stored one, or a
 * blank board with a one-step track. Read once — playground mode never touches
 * the store, and a different puzzle arrives as a fresh mount, not new props.
 *
 * A link brings the sharer's markers along with the board. They record someone
 * else's hints and refusals, so the board is adopted and the markers dropped —
 * every later save and summary on this device then counts only what's earned
 * here.
 */
function initialBoardState(
  puzzle: Puzzle,
  initialHash: string | null | undefined,
  ephemeral: boolean | undefined,
) {
  const questionCount = puzzle.questions.length;
  const saved = initialHash
    ? decodeShareHash(initialHash, questionCount)
    : ephemeral
      ? null
      : loadState(puzzle.id, questionCount);
  if (saved && saved.history.length > 0) {
    return initialHash
      ? { ...saved, hints: new Map<number, HintMarker>(), fails: new Map<number, FailMarker>() }
      : saved;
  }
  const blank = freshBoard(puzzle);
  return {
    questions: blank,
    completed: false,
    stale: false,
    history: [cloneStates(blank)],
    historyIdx: 0,
    hints: new Map<number, HintMarker>(),
    fails: new Map<number, FailMarker>(),
  };
}

interface PuzzleViewProps {
  puzzle: Puzzle;
  dateStr: string;
  level: number;
  initialHash?: string | null;
  /** Playground mode: render from the URL only, never touch localStorage. */
  ephemeral?: boolean;
  /** Filled with the view's share dialog, for the page's Share menu item to open. */
  shareRef?: { current: { open: () => void } | null };
  onNextPuzzle: () => void;
  onChanged: () => void;
}

export function PuzzleView({
  puzzle,
  dateStr,
  level,
  initialHash,
  ephemeral,
  shareRef,
  onNextPuzzle,
  onChanged,
}: PuzzleViewProps) {
  const s = t();
  const debugMode = debugEnabled();

  // Ephemeral (playground) mode persists nothing: the puzzle is fully described
  // by the URL. Gating the saveState calls is sufficient — saveMeta / loadMeta
  // all no-op without an existing entry, and loadState returns null, so no
  // other store touchpoint can write.

  // Resolved before the first paint, so the stored board never flickers in.
  const [initState] = useState(() => initialBoardState(puzzle, initialHash, ephemeral));

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
  // The undo stack lives in refs, not state: `foldMarkers` and `pushHistory`
  // rewrite it in place, and render reads it directly for the toolbar's enabled
  // set. `forceHistoryUpdate` is what repaints after a mutation.
  const historyRef = useRef<QuestionState[][]>(initState.history);
  const historyIdxRef = useRef(initState.historyIdx);
  const forceHistoryUpdate = useForceUpdate();

  const tabStateRef = useRef({
    started: initState.history.length > 1,
    completed: initState.completed,
    stale: initState.stale,
  });
  // Whether this session made any local change. v1 share URLs carry no
  // completed flag — completion is derived once wasm loads — so this is what
  // separates "you solved it" from "it arrived solved".
  const interactedRef = useRef(false);
  /**
   * A board can arrive already complete without being recorded as such — a
   * shared board that's solved, or a save predating a format change — and only
   * wasm can tell. The verdict is read once, off the board as it arrived: left
   * reacting to live validity it would fire again the moment the player solves,
   * writing this mount snapshot over the history they just built.
   */
  const arrivalRecorded = useRef(false);
  useEffect(() => {
    if (arrivalRecorded.current || ephemeral || !handleReady) return;
    arrivalRecorded.current = true;
    if (interactedRef.current || initState.completed) return;
    if (!validity.every(isValid)) return;
    saveState(puzzle.id, { ...initState, completed: true, stale: false });
    onChanged();
  }, [handleReady, validity, initState, puzzle.id, onChanged, ephemeral]);
  const historyBurstRef = useRef({ lastTime: 0 });

  function trackHistoryBurst() {
    const now = Date.now();
    if (now - historyBurstRef.current.lastTime > 15_000) {
      analytics.meta.current.historyBursts++;
      saveMeta(puzzle.id, analytics.meta.current);
    }
    historyBurstRef.current.lastTime = now;
  }

  /**
   * The Checkpoint button's verdict. Shares the hint's slot; only one speaks.
   * Goes on the next board change, a click, or after a stretch on screen.
   */
  const [checkpointNote, setCheckpointNote] = useState<string | null>(null);
  useVisibleTimeout(checkpointNote, NOTE_MS, () => setCheckpointNote(null));

  const [shareMode, setShareMode] = useState<ShareMode | null>(null);
  // Remounts the sheet, so a press while it is open returns it to Puzzle.
  const [sharePress, setSharePress] = useState(0);
  useEffect(() => {
    if (!shareRef) return undefined;
    shareRef.current = {
      open: () => {
        setShareMode("puzzle");
        setSharePress((n) => n + 1);
      },
    };
    return () => {
      shareRef.current = null;
    };
  }, [shareRef]);
  // Celebrating the solve just made, or summarizing a stored one from the bar.
  const [solvedDialog, setSolvedDialog] = useState<"celebrate" | "summary" | null>(null);

  const [focusedQuestion, setFocusedQuestionRaw] = useState<number | null>(null);
  const [focusedOption, setFocusedOptionRaw] = useState<number | null>(null);
  const focusedQuestionRef = useRef<number | null>(null);
  const focusedOptionRef = useRef<number | null>(null);
  const gridRef = useRef<HTMLDivElement>(null);
  const nextPuzzleRef = useRef<HTMLElement | null>(null);
  // A callback ref types itself against whichever element the bar renders,
  // where one `useRef` would need casting at each of the two sites.
  const setNextPuzzleRef = useCallback((el: HTMLElement | null) => {
    nextPuzzleRef.current = el;
  }, []);
  const puzzleCompleteRef = useRef<HTMLDivElement>(null);
  // The two buttons a nudge can point at.
  const checkpointBtnRef = useRef<HTMLButtonElement>(null);
  const hintBtnRef = useRef<HTMLButtonElement>(null);
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

  /**
   * Markers on steps a history rewrite discards fold onto the branch point —
   * the step just before the newly added one — rather than being deleted, so
   * no rewind can erase the record. Refusal counts add up and their questions
   * union; hint markers keep the deepest level reached, since that's what the
   * value means, and the earlier marker's question stands.
   */
  function foldMarkers(branchIdx: number) {
    for (const [key, hint] of [...hintMarkers.current]) {
      if (key <= branchIdx) continue;
      hintMarkers.current.delete(key);
      const onto = hintMarkers.current.get(branchIdx);
      hintMarkers.current.set(branchIdx, {
        level: Math.max(onto?.level ?? 0, hint.level),
        qi: onto?.qi ?? hint.qi,
      });
    }
    for (const [key, fail] of [...failMarkers.current]) {
      if (key <= branchIdx) continue;
      failMarkers.current.delete(key);
      const onto = failMarkers.current.get(branchIdx);
      failMarkers.current.set(branchIdx, {
        count: (onto?.count ?? 0) + fail.count,
        qis: [...new Set([...(onto?.qis ?? []), ...fail.qis])],
      });
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

  const hintMarkers = useRef<Map<number, HintMarker>>(initState.hints);
  /**
   * Refused checkpoint presses, keyed by the step the press landed on. History
   * rewrites can't shake them off — `foldMarkers` slides them onto the branch
   * point instead of deleting them. `meta.checkpointFails` tallies the same
   * presses monotonically for scoring.
   */
  const failMarkers = useRef<Map<number, FailMarker>>(initState.fails);

  // Markers move while the board stands still, so `revalidate` never runs for
  // them — they save here instead. Completion and staleness are the board's, so
  // they carry over from the last sweep.
  function saveMarkers() {
    if (ephemeral) return;
    saveState(puzzle.id, {
      questions: questionsRef.current,
      completed: tabStateRef.current.completed,
      stale: tabStateRef.current.stale,
      history: historyRef.current,
      historyIdx: historyIdxRef.current,
      hints: hintMarkers.current,
      fails: failMarkers.current,
    });
  }

  function pushFailMarker(qi: number) {
    const map = failMarkers.current;
    const idx = historyIdxRef.current;
    const at = map.get(idx);
    map.set(idx, {
      count: (at?.count ?? 0) + 1,
      qis: [...new Set([...(at?.qis ?? []), qi])],
    });
    analytics.meta.current.checkpointFails++;
    saveMeta(puzzle.id, analytics.meta.current);
    saveMarkers();
    forceHistoryUpdate();
  }

  function pushHintMarker(hintLevel: number, qi: number | null) {
    const map = hintMarkers.current;
    const idx = historyIdxRef.current;
    // Deepest level reached at this step — a fresh press never lowers a level
    // that folded here from rewritten history. The question is whichever a
    // step named first and stays put once set.
    const at = map.get(idx);
    map.set(idx, {
      level: Math.max(at?.level ?? 0, hintLevel),
      qi: at?.qi ?? qi,
    });
    analytics.meta.current.hints++;
    saveMeta(puzzle.id, analytics.meta.current);
    saveMarkers();
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

  // L1-only ambient coach: calm intro text, idle nudges, and mistake notes,
  // with reference arrows. Silent the instant the player engages. Replaces the
  // old auto-solve tutorial; higher levels and playground get nothing.
  const coachEnabled = level === 1 && !ephemeral;
  const coachTextRef = useRef<HTMLDivElement>(null);
  // Held steady between marks so the overlay's geometry effect can depend on it.
  const coachMarks = useMemo(() => questions.map((q) => q.marks), [questions]);
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
    sweepTimer.current = window.setTimeout(() => setSweepMasks(null), SWEEP_MS);
  }
  useEffect(() => () => clearTimeout(sweepTimer.current), []);
  // Layout effect: cells must be placed before the first frame paints.
  useLayoutEffect(() => {
    if (sweepMasks !== null && gridRef.current) placeSweep(gridRef.current);
  }, [sweepMasks]);

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

  const hasProgress = historyRef.current.length > 1;
  // Nothing to verify on a blank board, and a checkpoint can't checkpoint itself.
  const canCheckpoint =
    historyIdxRef.current > 0 &&
    describeDiff(
      historyRef.current[historyIdxRef.current - 1],
      historyRef.current[historyIdxRef.current],
    ).qi >= 0;

  // L2+ only: the playground is level 1, and L1 has the coach instead.
  const nudge = useIdleNudge({
    enabled: level > 1 && hasProgress && !completed,
    canCheckpoint,
    progressKey: questions,
  });

  /**
   * Grant a checkpoint if nothing on the board contradicts the key. Marks the
   * last checkpoint verified are locked against editing, so a re-verified
   * range can only have grown — the one way to fail is a wrong new mark, which
   * lands a refusal marker and opens the mistake steps in the hint panel.
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
      pushFailMarker(mistake.qi);
      hints.showMistake(mistake, s.puzzle.checkpointWrong);
      return;
    }

    nudge.used("checkpoint");
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

  /** Back to a blank board and an empty track — the solve, and its record, go. */
  function handlePlayAgain() {
    const fresh = freshBoard(puzzle);
    historyRef.current = [cloneStates(fresh)];
    historyIdxRef.current = 0;
    // Explicit, or the next mark's foldMarkers would pile them all onto Start.
    hintMarkers.current = new Map();
    failMarkers.current = new Map();
    setQuestions(fresh);
    revalidate(fresh);
    hints.clear();
    setCheckpointNote(null);
    historyBurstRef.current.lastTime = 0;
    analytics.restart();
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

  function handleHint() {
    nudge.used("hint");
    hints.handleHint();
  }

  /** The board as it stands, for a progress link; null before the first mark. */
  function progressState() {
    if (!hasProgress) return null;
    return {
      questions,
      completed,
      stale: false,
      history: historyRef.current,
      historyIdx: historyIdxRef.current,
      hints: hintMarkers.current,
      fails: failMarkers.current,
    };
  }

  // Reports the solve and opens its summary.
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
    // The counters stay on the ledger past the solve, for the summary; this
    // write lands the flushed time.
    if (!ephemeral) saveMeta(puzzle.id, m);
    setSolvedDialog("celebrate");
    return undefined;
  }, [completed, level, puzzle.id, analytics.meta, analytics.wasCompleted, ephemeral]);

  // Re-seed the toolbar's roving tabindex whenever its enabled set changes;
  // between those the arrow keys' own position stands.
  useEffect(() => {
    initRovingTabindex(controlsRef.current, "button:not(:disabled)");
  }, [completed, canUndo, canRedo, canCheckpoint]);

  // The strip's buttons come and go with the history and with the range it has
  // collapsed, neither of which render declares — so this re-seeds every time.
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

  function moveFocus(questionDelta: number, optionDelta: number) {
    const qi = focusedQuestionRef.current ?? 0;
    const oi = focusedOptionRef.current ?? 0;
    const nextQi = (qi + questionDelta + questionCount) % questionCount;
    let nextOi = (oi + optionDelta + 5) % 5;
    // When moving between questions, snap to the correct option if the
    // target option is disabled (another option is marked correct)
    if (questionDelta !== 0) {
      const marks = questionsRef.current[nextQi]?.marks;
      if (marks) {
        const correctIdx = marks.indexOf("correct");
        if (correctIdx >= 0) nextOi = correctIdx;
      }
    }
    setFocusedQuestion(nextQi);
    setFocusedOption(nextOi);
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

  // The window shortcuts are bound once and outlive the render that armed them,
  // so they call the current handlers through a ref rather than re-binding
  // every one of them on every render.
  const keyActions = {
    markOption: handleOptionClick,
    digit: handleDigit,
    undo: handleUndo,
    redo: handleRedo,
    checkpoint: handleSave,
    moveFocus,
    hint: handleHint,
    completed,
  };
  const keyActionsRef = useRef(keyActions);
  keyActionsRef.current = keyActions;

  useEffect(() => {
    // Every shortcut is inert once the board is solved.
    const whileSolving = (fn: (ev: KeyboardEvent) => void) =>
      guarded((ev) => {
        if (!keyActionsRef.current.completed) fn(ev);
      });
    // The browser's own undo stays out of the way whether or not the board is
    // still live, so this one preventDefaults ahead of the solved check.
    const undoRedo = (step: () => void) =>
      guarded((ev) => {
        ev.preventDefault();
        if (!keyActionsRef.current.completed) step();
      });

    const bindings: Record<string, (ev: KeyboardEvent) => void> = {
      h: whileSolving(() => keyActionsRef.current.hint()),
      p: whileSolving(() => keyActionsRef.current.checkpoint()),
      j: whileSolving(() => keyActionsRef.current.moveFocus(1, 0)),
      k: whileSolving(() => keyActionsRef.current.moveFocus(-1, 0)),
      "$mod+z": undoRedo(() => keyActionsRef.current.undo()),
      "$mod+Shift+z": undoRedo(() => keyActionsRef.current.redo()),
      "$mod+y": undoRedo(() => keyActionsRef.current.redo()),
    };
    // An option letter marks that option on the focused question; a digit feeds
    // the question-number buffer.
    OPTION_KEYS.forEach((key, oi) => {
      bindings[key] = whileSolving(() => {
        const qi = focusedQuestionRef.current;
        if (qi != null) keyActionsRef.current.markOption(qi, oi);
      });
    });
    for (let digit = 0; digit <= 9; digit++) {
      bindings[String(digit)] = whileSolving(() => keyActionsRef.current.digit(digit));
    }
    return tinykeys(window, bindings);
  }, []);

  return (
    <>
      <div class="puzzle-view">
        {coachEnabled && !completed && <CoachText message={coach.message} boxRef={coachTextRef} />}
        {/* Questions */}
        <div
          ref={gridRef}
          class={classNames("questions-grid", puzzle.questions.length <= 3 && "single-col")}
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
            marks={coachMarks}
            optionCount={puzzle.optionCount}
          />
        )}

        {/* Idle nudge: a coach message and arrow. Any press takes it away. */}
        {!completed && nudge.kind && (
          <NudgeCallout
            text={s.puzzle.nudge[nudge.kind]}
            targetRef={nudge.kind === "checkpoint" ? checkpointBtnRef : hintBtnRef}
            kind={nudge.kind}
            showMs={nudge.showMs}
          />
        )}

        <div class="puzzle-dock">
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
          {!completed && checkpointNote && (
            <div class="puzzle-note" role="status" onClick={() => setCheckpointNote(null)}>
              <span>{checkpointNote}</span>
              <button
                class="note-dismiss"
                aria-label={s.aria.dismiss}
                onClick={() => setCheckpointNote(null)}
              >
                &times;
              </button>
            </div>
          )}

          {/* Controls and the history track share a line while the track is
              short; a long track wraps onto its own. Solved, the controls go
              and the completion bar stands at the row's end instead. */}
          <div class="puzzle-dock-row">
            {!completed && (
              <div
                ref={controlsRef}
                class="puzzle-controls"
                role="toolbar"
                onKeyDown={arrowNavHandler("button:not(:disabled)")}
              >
                <button
                  class="toolbar-icon-btn"
                  onClick={handleUndo}
                  disabled={!canUndo}
                  title={s.puzzle.undo}
                >
                  <IconUndo />
                </button>
                <button
                  class="toolbar-icon-btn"
                  onClick={handleRedo}
                  disabled={!canRedo}
                  title={s.puzzle.redo}
                >
                  <IconRedo />
                </button>
                <button
                  ref={checkpointBtnRef}
                  class="toolbar-accent-btn"
                  onClick={handleSave}
                  disabled={!canCheckpoint}
                >
                  <IconPin size="0.9em" class="icon-checkpoint" /> {s.puzzle.checkpoint}
                </button>
                <button
                  ref={hintBtnRef}
                  class="toolbar-accent-btn"
                  onClick={handleHint}
                  onMouseEnter={hints.getSolution}
                  onFocus={hints.getSolution}
                  onTouchStart={hints.getSolution}
                  title={s.puzzle.hint}
                >
                  <IconHint size="0.9em" class="icon-hint" /> {s.puzzle.hint}
                </button>
              </div>
            )}

            {historyRef.current.length > 1 && (
              <HistoryStrip
                history={historyRef.current}
                currentIdx={historyIdxRef.current}
                hints={hintMarkers.current}
                fails={failMarkers.current}
                completed={completed}
                onJump={handleJumpTo}
                onPlayAgain={handlePlayAgain}
                containerRef={historyStripRef}
              />
            )}

            {/* The completion bar: the ways onward, at the end of the row.
                The dialog carries the same two while it is up. */}
            {completed && (
              <div
                ref={puzzleCompleteRef}
                class={classNames("puzzle-complete", solvedDialog && "quiet")}
                aria-label={s.puzzle.solved}
              >
                <button class="toolbar-accent-btn" onClick={() => setSolvedDialog("summary")}>
                  {s.puzzle.summary}
                </button>
                {level < LEVELS.length ? (
                  <button ref={setNextPuzzleRef} class="next-puzzle-btn" onClick={onNextPuzzle}>
                    {s.puzzle.nextPuzzle} &rarr;
                  </button>
                ) : (
                  <a ref={setNextPuzzleRef} href="/archive" class="next-puzzle-btn">
                    {s.daily.archive} &rarr;
                  </a>
                )}
              </div>
            )}
          </div>
        </div>
      </div>
      {shareMode && (
        <PuzzleShareDialog
          key={sharePress}
          dateStr={dateStr}
          level={level}
          initialMode={shareMode}
          getProgress={progressState}
          onClose={() => setShareMode(null)}
        />
      )}
      {solvedDialog && (
        <SolvedDialog
          stats={
            solvedDialog === "celebrate"
              ? analytics.meta.current
              : storedSolveStats(
                  ephemeral ? null : loadMeta(puzzle.id),
                  historyRef.current,
                  hintMarkers.current,
                  failMarkers.current,
                )
          }
          dateStr={dateStr}
          level={level}
          outcomes={questionOutcomes(
            puzzle.questions.length,
            hintMarkers.current,
            failMarkers.current,
          )}
          hasNext={level < LEVELS.length}
          shareUrl={getPuzzleUrl(dateStr, level)}
          celebrate={solvedDialog === "celebrate"}
          onNext={onNextPuzzle}
          onClose={() => {
            setSolvedDialog(null);
            // The bar takes the loud copy back on this render; bring it into
            // view and focus its button once it has.
            requestAnimationFrame(() => {
              puzzleCompleteRef.current?.scrollIntoView({ behavior: "smooth", block: "nearest" });
              nextPuzzleRef.current?.focus({ preventScroll: true });
            });
          }}
        />
      )}
    </>
  );
}
