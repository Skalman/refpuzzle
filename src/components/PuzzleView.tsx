import { useState, useEffect, useCallback, useRef } from "preact/hooks";
import { tinykeys } from "tinykeys";
import type { Marks, Puzzle } from "../engine/types.ts";
import { FRESH_MARKS, getFlatPuzzle } from "../engine/types.ts";
import { checkAnswerValidity } from "../engine/check-validity.ts";
import { deriveState } from "../engine/state.ts";
import type { Validity } from "../engine/state.ts";
import { loadState, saveState, saveMeta, clearMeta, cloneStates } from "../lib/store.ts";
import type { QuestionState } from "../lib/store.ts";
import { decodeShareHash, getShareUrl, getPuzzleUrl } from "../lib/share.ts";
import { guarded, arrowNavHandler, initRovingTabindex } from "../lib/keyboard.ts";
import { confetti } from "../lib/confetti.ts";
import { track, getClientInfo } from "../lib/analytics.ts";
import { t } from "../i18n/index.ts";
import { QuestionRow } from "./QuestionRow.tsx";
import { HistoryStrip, describeDiff } from "./HistoryStrip.tsx";
import { HintStep } from "./HintStep.tsx";
import { TutorialOverlay } from "./TutorialOverlay.tsx";
import { TutorialWelcome } from "./TutorialWelcome.tsx";
import { TutorialHighlightCtx } from "./TutorialHighlight.ts";
import { useTutorial } from "./useTutorial.ts";
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
  onNextPuzzle: () => void;
  onChanged: () => void;
  onStartTutorial?: () => void;
  autoStartTutorial?: boolean;
  onTutorialConsumed?: () => void;
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
  const [validity, setValidity] = useState<Validity[]>(() => {
    const fp = getFlatPuzzle(puzzle);
    const { answers, eliminated } = deriveState(
      initState.questions.map((q) => q.marks),
      puzzle.optionCount,
    );
    return answers.map((_a, qi) => checkAnswerValidity(fp, answers, eliminated, qi));
  });
  const historyRef = useRef<QuestionState[][]>(initState.history);
  const historyIdxRef = useRef(initState.historyIdx);
  const forceHistoryUpdate = useForceUpdate();

  const historyBurstRef = useRef({ lastTime: 0 });
  const tutorialReachedEnd = useRef(false);

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
  const numberBuf = useRef({ digits: "", timer: 0 });
  const controlsRef = useRef<HTMLDivElement>(null);
  const historyStripRef = useRef<HTMLDivElement>(null);
  const tutorialHeadingRef = useRef<HTMLDivElement>(null);

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
        forceHistoryUpdate();
        return;
      }
    }

    historyRef.current.push(cloned);
    historyIdxRef.current = historyRef.current.length - 1;
    forceHistoryUpdate();
  }

  const hintMarkers = useRef<Map<number, number>>(initState.hints);

  function pushHintMarker(hintLevel: number) {
    hintMarkers.current.set(historyIdxRef.current, hintLevel);
    analytics.meta.current.hints++;
    saveMeta(puzzle.id, analytics.meta.current);
    forceHistoryUpdate();
  }

  const revalidate = useCallback(
    (qs: QuestionState[]) => {
      const fp = getFlatPuzzle(puzzle);
      const { answers, eliminated } = deriveState(
        qs.map((q) => q.marks),
        puzzle.optionCount,
      );
      const result: Validity[] = answers.map((_a, qi) =>
        checkAnswerValidity(fp, answers, eliminated, qi),
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
      if (analytics.wasStarted.current) saveMeta(puzzle.id, analytics.meta.current);
      onChanged();
    },
    [puzzle, onChanged, analytics.meta, analytics.wasStarted],
  );

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
    if (tutorial.active) {
      tutorialHeadingRef.current?.scrollIntoView({ behavior: "smooth", block: "start" });
    }
  }, [tutorial.active]);

  useEffect(() => {
    if (questions.length > 0 && !tutorial.active) revalidate(questions);
  }, [questions, revalidate, tutorial.active]);

  const completed = validity.length > 0 && validity.every((v) => v === "valid");
  const completedRef = useRef(completed);
  completedRef.current = completed;
  const canUndo = historyIdxRef.current > 0;
  const canRedo = historyIdxRef.current < historyRef.current.length - 1;

  const hints = useHintEngine(puzzle, {
    questionsRef,
    debugMode,
    pushHintMarker,
    completed,
    questions,
  });

  function applyChange(next: QuestionState[]) {
    analytics.markStarted();
    pushHistory(next);
    setQuestions(next);
    hints.clear();
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
    trackHistoryBurst();
    historyIdxRef.current--;
    setQuestions(cloneStates(historyRef.current[historyIdxRef.current]));
    hints.clear();
    forceHistoryUpdate();
    focusCurrentStep();
  }

  function handleRedo() {
    if (historyIdxRef.current >= historyRef.current.length - 1) return;
    trackHistoryBurst();
    historyIdxRef.current++;
    setQuestions(cloneStates(historyRef.current[historyIdxRef.current]));
    hints.clear();
    forceHistoryUpdate();
    focusCurrentStep();
  }

  function handleJumpTo(idx: number) {
    if (idx < 0 || idx >= historyRef.current.length) return;
    trackHistoryBurst();
    historyIdxRef.current = idx;
    setQuestions(cloneStates(historyRef.current[idx]));
    hints.clear();
    forceHistoryUpdate();
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
      forceHistoryUpdate();
    } else {
      analytics.meta.current.checkpoints++;
      saveMeta(puzzle.id, analytics.meta.current);
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
    hints.clear();
    analytics.wasCompleted.current = false;
  }

  const hasProgress = historyRef.current.length > 1;

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
        history: historyRef.current,
        historyIdx: historyIdxRef.current,
        hints: hintMarkers.current,
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
    if (!completed || analytics.wasCompleted.current || tutorial.active) return undefined;
    analytics.wasCompleted.current = true;
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
      ...(m.hints > 0 && { hints: m.hints }),
      ...(m.checkpoints > 0 && { checkpoints: m.checkpoints }),
      ...(m.historyBursts > 0 && { historyBursts: m.historyBursts }),
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
  }, [completed, level, puzzle.id, tutorial.active, analytics.meta, analytics.wasCompleted]);

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
      {(tutorial.active || tutorial.welcome) && (
        <div
          class={`tutorial-scrim${tutorial.welcome && !tutorial.active ? " tutorial-scrim-welcome" : ""}`}
        />
      )}
      <div class={`puzzle-view${tutorial.active || tutorial.welcome ? " tutorial-active" : ""}`}>
        {tutorial.active && (
          <div ref={tutorialHeadingRef} class="tutorial-heading">
            Tutorial
          </div>
        )}
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
            onDismiss={() => {
              if (!tutorialReachedEnd.current) {
                track("tutorial_dismissed", {
                  puzzleId: puzzle.id,
                  level,
                  ...getClientInfo(),
                });
              }
              tutorialReachedEnd.current = false;
              tutorial.dismiss();
            }}
            onSetHighlight={tutorial.setHighlight}
            onApplyStep={tutorial.applyStep}
            onUnapplyStep={tutorial.unapplyStep}
            onDone={() => {
              tutorial.setHighlight(null);
              tutorialReachedEnd.current = true;
              track("tutorial_completed", {
                puzzleId: puzzle.id,
                level,
                ...getClientInfo(),
              });
            }}
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
            onClick={hints.handleHint}
            onMouseEnter={hints.getSolution}
            onFocus={hints.getSolution}
            onTouchStart={hints.getSolution}
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
