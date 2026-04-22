import { useState, useEffect, useCallback, useRef } from "preact/hooks";
import type { AnswerLetter, Marks, Puzzle } from "../engine/types.ts";
import { LETTERS } from "../engine/types.ts";
import { validate } from "../engine/validate.ts";
import { findHint } from "../engine/hints.ts";
import { loadState, saveState } from "../lib/store.ts";
import type { QuestionState } from "../lib/store.ts";
import { decodeShareHash, getShareUrl, getPuzzleUrl, sharePuzzleLink } from "../lib/share.ts";
import { t } from "../i18n/index.ts";
import { QuestionRow } from "./QuestionRow.tsx";

const FRESH_MARKS: Marks = ["unmarked", "unmarked", "unmarked", "unmarked", "unmarked"];


function cloneStates(qs: QuestionState[]): QuestionState[] {
  return qs.map((q) => ({ marks: [...q.marks] as Marks }));
}

interface MoveInfo {
  label: string;
  qi: number;
  oi: number;
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
      let label: string;
      let priority: number;
      if (n === "correct") {
        label = `Q${qi + 1}=${letter} \u2705`;
        priority = 2;
      } else if (n === "incorrect") {
        label = `Q${qi + 1} ${letter} \u274C`;
        priority = 1;
      } else {
        label = `Q${qi + 1} ${letter} undo`;
        priority = 0;
      }
      if (priority > bestPriority) {
        best = { label, qi, oi };
        bestPriority = priority;
      }
    }
  }
  if (!best) {
    return { label: "\u{1F4CC}", qi: -1, oi: -1 };
  }
  if (bestPriority === 0) {
    const allUnmarked = next.every((q) => q.marks.every((m) => m === "unmarked"));
    if (allUnmarked) return { label: "reset", qi: -1, oi: -1 };
  }
  return best;
}

function HistoryStrip({
  history,
  currentIdx,
  onJump,
}: {
  history: QuestionState[][];
  currentIdx: number;
  onJump: (idx: number) => void;
}) {
  if (history.length <= 1) return null;

  const moves: MoveInfo[] = [];
  for (let i = 1; i < history.length; i++) {
    moves.push(describeDiff(history[i - 1], history[i]));
  }

  return (
    <div class="history-strip">
      <button
        class={`history-step ${currentIdx === 0 ? "current" : ""}`}
        onClick={() => onJump(0)}
      >
        start
      </button>
      {moves.map((move, i) => (
        <button
          key={`${move.qi}-${move.oi}-${move.label}`}
          class={`history-step ${i + 1 === currentIdx ? "current" : ""} ${i + 1 > currentIdx ? "future" : ""} ${move.label === "\u{1F4CC}" ? "checkpoint" : ""}`}
          onClick={() => onJump(i + 1)}
          title={move.label}
        >
          {move.label}
        </button>
      ))}
    </div>
  );
}

function Toast({ message, onDone }: { message: string; onDone: () => void }) {
  useEffect(() => {
    const timer = setTimeout(onDone, 2000);
    return () => clearTimeout(timer);
  }, [onDone]);

  return <div class="toast">{message}</div>;
}

interface PuzzleViewProps {
  puzzle: Puzzle;
  dateStr: string;
  level: number;
  initialHash?: string | null;
  onNextPuzzle: () => void;
  onCompleted: () => void;
}

export function PuzzleView({ puzzle, dateStr, level, initialHash, onNextPuzzle, onCompleted }: PuzzleViewProps) {
  const s = t();

  const [questions, setQuestions] = useState<QuestionState[]>([]);
  const [validity, setValidity] = useState<("neutral" | "valid" | "invalid")[]>([]);
  const [hintText, setHintText] = useState<string | null>(null);
  const hintRef = useRef<{ steps: string[]; step: number } | null>(null);

  const historyRef = useRef<QuestionState[][]>([]);
  const historyIdxRef = useRef(-1);
  const [_historyVersion, setHistoryVersion] = useState(0);

  const [resetPending, setResetPending] = useState(false);
  const [toastMessage, setToastMessage] = useState<string | null>(null);

  function pushHistory(qs: QuestionState[]) {
    const h = historyRef.current;
    const idx = historyIdxRef.current;
    historyRef.current = h.slice(0, idx + 1);

    const cloned = cloneStates(qs);
    if (historyRef.current.length >= 2) {
      const prev = historyRef.current[historyRef.current.length - 2];
      const last = historyRef.current[historyRef.current.length - 1];
      const lastDiff = describeDiff(prev, last);
      const newDiff = describeDiff(last, cloned);
      if (lastDiff.qi >= 0 && lastDiff.qi === newDiff.qi && lastDiff.oi === newDiff.oi) {
        historyRef.current[historyRef.current.length - 1] = cloned;
        setHistoryVersion((v) => v + 1);
        return;
      }
    }

    historyRef.current.push(cloned);
    historyIdxRef.current = historyRef.current.length - 1;
    setHistoryVersion((v) => v + 1);
  }

  useEffect(() => {
    const n = puzzle.questions.length;

    // Check for shared state hash, then localStorage
    const saved = initialHash
      ? decodeShareHash(initialHash, n)
      : loadState(puzzle.id);

    if (saved && saved.history.length > 0) {
      historyRef.current = saved.history;
      historyIdxRef.current = saved.historyIdx;
      setQuestions(saved.questions);
      return;
    }

    const blank = puzzle.questions.map(() => ({ marks: [...FRESH_MARKS] as Marks }));
    setQuestions(blank);
    historyRef.current = [cloneStates(blank)];
    historyIdxRef.current = 0;
  }, [puzzle, initialHash]);

  const revalidate = useCallback(
    (qs: QuestionState[]) => {
      const answers: (AnswerLetter | null)[] = qs.map((q) => {
        const idx = q.marks.indexOf("correct");
        return idx >= 0 ? LETTERS[idx] : null;
      });
      const result = validate(puzzle, answers);
      setValidity(result);

      const isCompleted = result.every((v) => v === "valid");
      saveState(puzzle.id, {
        questions: qs,
        completed: isCompleted,
        history: historyRef.current,
        historyIdx: historyIdxRef.current,
      });
      if (isCompleted) {
        onCompleted();
      }
    },
    [puzzle, onCompleted],
  );

  useEffect(() => {
    if (questions.length > 0) revalidate(questions);
  }, [questions, revalidate]);

  const completed = validity.length > 0 && validity.every((v) => v === "valid");
  const canUndo = historyIdxRef.current > 0;
  const canRedo = historyIdxRef.current < historyRef.current.length - 1;

  function applyChange(next: QuestionState[]) {
    pushHistory(next);
    setQuestions(next);
    setHintText(null);
    hintRef.current = null;
  }

  function handleOptionClick(questionIdx: number, optionIdx: number) {
    const next = cloneStates(questions);
    const q = next[questionIdx];
    const current = q.marks[optionIdx];

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
  }

  function handleUndo() {
    if (!canUndo) return;
    historyIdxRef.current--;
    setQuestions(cloneStates(historyRef.current[historyIdxRef.current]));
    setHintText(null);
    hintRef.current = null;
    setHistoryVersion((v) => v + 1);
  }

  function handleRedo() {
    if (!canRedo) return;
    historyIdxRef.current++;
    setQuestions(cloneStates(historyRef.current[historyIdxRef.current]));
    setHintText(null);
    hintRef.current = null;
    setHistoryVersion((v) => v + 1);
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
    pushHistory(cloneStates(questions));
  }

  function handleReset() {
    if (!resetPending) {
      setResetPending(true);
      return;
    }
    setResetPending(false);
    const fresh = puzzle.questions.map(() => ({
      marks: [...FRESH_MARKS] as Marks,
    }));
    historyRef.current = [cloneStates(fresh)];
    historyIdxRef.current = 0;
    setQuestions(fresh);
    setHintText(null);
    hintRef.current = null;
  }

  function handleHint() {
    if (hintRef.current && hintRef.current.step < hintRef.current.steps.length - 1) {
      hintRef.current.step++;
      setHintText(hintRef.current.steps[hintRef.current.step]);
      return;
    }

    const markSets = questions.map((q) => q.marks);
    const result = findHint(puzzle, markSets);
    if (result) {
      hintRef.current = { steps: result.steps, step: 0 };
      setHintText(result.steps[0]);
    } else {
      hintRef.current = null;
      setHintText(null);
    }
  }

  async function handleShare() {
    const hasProgress = questions.some((q) => q.marks.some((m) => m !== "unmarked"));
    const url = hasProgress
      ? getShareUrl(dateStr, level, {
          questions,
          completed,
          history: historyRef.current,
          historyIdx: historyIdxRef.current,
        })
      : getPuzzleUrl(dateStr, level);

    const ok = await sharePuzzleLink(url, `${puzzle.title} - Refpuzzle`);
    if (ok) {
      setToastMessage(s.puzzle.linkCopied);
    }
  }

  // Clear reset pending after timeout
  useEffect(() => {
    if (!resetPending) return undefined;
    const timer = setTimeout(() => setResetPending(false), 3000);
    return () => clearTimeout(timer);
  }, [resetPending]);

  return (
    <div class="puzzle-view">
      {/* Questions */}
      <div class={puzzle.difficulty >= 3 ? "questions-grid" : ""}>
        {puzzle.questions.map((qDef, qi) => (
          <QuestionRow
            key={qDef.text}
            index={qi}
            question={qDef}
            marks={questions[qi]?.marks ?? FRESH_MARKS}
            validity={validity[qi] ?? "neutral"}
            onOptionClick={(oi) => handleOptionClick(qi, oi)}
          />
        ))}
      </div>

      {/* Hint display */}
      {hintText && !completed && (
        <div class="puzzle-hint">
          {hintText}
          {hintRef.current && hintRef.current.step < hintRef.current.steps.length - 1 && (
            <button class="hint-more" onClick={handleHint}>
              more
            </button>
          )}
        </div>
      )}

      {/* Completion banner */}
      {completed && (
        <div class="puzzle-complete">
          <span>{s.puzzle.solved}</span>
          <button class="next-puzzle-btn" onClick={onNextPuzzle}>
            {s.puzzle.nextPuzzle} &rarr;
          </button>
        </div>
      )}

      {/* Controls */}
      <div class="puzzle-controls">
        <button class="toolbar-icon-btn" onClick={handleUndo} disabled={!canUndo} title={s.puzzle.undo}>&#x21A9;</button>
        <button class="toolbar-icon-btn" onClick={handleRedo} disabled={!canRedo} title={s.puzzle.redo}>&#x21AA;</button>
        <button class="toolbar-accent-btn" onClick={handleSave}>&#x1F4CC; Checkpoint</button>
        <span class="controls-spacer"></span>
        <button class="toolbar-accent-btn" onClick={handleHint} title={s.puzzle.hint}>&#x1F4A1; {s.puzzle.hint}</button>
        <button class="toolbar-accent-btn" onClick={handleShare} title={s.puzzle.share}>{s.puzzle.share}</button>
        <button
          class={`toolbar-accent-btn ${resetPending ? "reset-confirm" : ""}`}
          onClick={handleReset}
          disabled={historyRef.current.length <= 1}
        >
          {resetPending ? s.puzzle.resetConfirm : s.puzzle.reset}
        </button>
      </div>

      
      {historyRef.current.length > 1 && (
        <HistoryStrip
          history={historyRef.current}
          currentIdx={historyIdxRef.current}
          onJump={handleJumpTo}
        />
      )}

      {/* Toast notification */}
      {toastMessage && <Toast message={toastMessage} onDone={() => setToastMessage(null)} />}
    </div>
  );
}
