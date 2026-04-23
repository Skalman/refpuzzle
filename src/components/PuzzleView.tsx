import { useState, useEffect, useCallback, useRef } from "preact/hooks";
import type { AnswerLetter, Marks, Puzzle } from "../engine/types.ts";
import { LETTERS } from "../engine/types.ts";
import { validate } from "../engine/validate.ts";
import type { Validity } from "../engine/validate.ts";
import { findHint } from "../engine/hints.ts";
import { loadState, saveState } from "../lib/store.ts";
import type { QuestionState } from "../lib/store.ts";
import {
  decodeShareHash,
  getShareUrl,
  getPuzzleUrl,
  sharePuzzleLink,
} from "../lib/share.ts";
import { t } from "../i18n/index.ts";
import { QuestionRow } from "./QuestionRow.tsx";

const FRESH_MARKS: Marks = [
  "unmarked",
  "unmarked",
  "unmarked",
  "unmarked",
  "unmarked",
];

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
        text = `Q${qi + 1}=${letter}`;
        icon = "\u2705";
        priority = 2;
      } else if (n === "incorrect") {
        text = `Q${qi + 1} ${letter}`;
        icon = "\u274C";
        priority = 1;
      } else {
        text = `Q${qi + 1} ${letter}`;
        icon = "\u21A9";
        priority = 0;
      }
      if (priority > bestPriority) {
        best = { text, icon, qi, oi };
        bestPriority = priority;
      }
    }
  }
  if (!best) {
    return { text: "", icon: "\u{1F4CC}", qi: -1, oi: -1 };
  }
  if (bestPriority === 0) {
    const allUnmarked = next.every((q) =>
      q.marks.every((m) => m === "unmarked"),
    );
    if (allUnmarked) return { text: "reset", icon: "", qi: -1, oi: -1 };
  }
  return best;
}

function HistoryStrip({
  history,
  currentIdx,
  hints,
  onJump,
}: {
  history: QuestionState[][];
  currentIdx: number;
  hints: Map<number, number>;
  onJump: (idx: number) => void;
}) {
  if (history.length <= 1) return null;

  const moves: MoveInfo[] = [];
  for (let i = 1; i < history.length; i++) {
    moves.push(describeDiff(history[i - 1], history[i]));
  }

  let lastCp = -1;
  for (let i = Math.min(currentIdx - 1, moves.length - 1); i >= 0; i--) {
    if (moves[i].qi < 0 && moves[i].icon) { lastCp = i; break; }
  }

  return (
    <div class="history-strip">
      <button
        class={`history-step ${currentIdx === 0 ? "current" : ""}`}
        onClick={() => onJump(0)}
      >
        start
      </button>
      {moves.map((move, i) => {
        const stepIdx = i + 1;
        const hintLevel = hints.get(stepIdx);
        const isCheckpoint = move.qi < 0 && move.icon;
        return (
          // oxlint-disable-next-line react/no-array-index-key
          <span key={i} class="history-entry">
            <button
              class={`history-step ${stepIdx === currentIdx ? "current" : ""} ${stepIdx > currentIdx ? "future" : ""} ${isCheckpoint && i === lastCp ? "checkpoint" : ""} ${isCheckpoint && i !== lastCp ? "checkpoint-old" : ""}`}
              onClick={() => onJump(stepIdx)}
              title={move.text}
            >
              {move.icon && <span class="history-icon">{move.icon} </span>}{move.text}
            </button>
            {hintLevel != null && <span class="history-hint">💡{hintLevel}</span>}
          </span>
        );
      })}
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
  onChanged: () => void;
}

export function PuzzleView({
  puzzle,
  dateStr,
  level,
  initialHash,
  onNextPuzzle,
  onChanged,
}: PuzzleViewProps) {
  const s = t();

  const [questions, setQuestionsRaw] = useState<QuestionState[]>([]);
  const questionsRef = useRef<QuestionState[]>([]);
  function setQuestions(qs: QuestionState[]) {
    questionsRef.current = qs;
    setQuestionsRaw(qs);
  }
  const [validity, setValidity] = useState<Validity[]>(
    [],
  );
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
      if (
        lastDiff.qi >= 0 &&
        lastDiff.qi === newDiff.qi &&
        lastDiff.oi === newDiff.oi
      ) {
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

  const hintMarkers = useRef<Map<number, number>>(new Map());

  function pushHintMarker(hintLevel: number) {
    hintMarkers.current.set(historyIdxRef.current, hintLevel);
    setHistoryVersion((v) => v + 1);
  }

  useEffect(() => {
    const n = puzzle.questions.length;

    // Check for shared state hash, then localStorage
    const saved = initialHash
      ? decodeShareHash(initialHash, n)
      : loadState(puzzle.id, n);

    if (saved && saved.history.length > 0) {
      historyRef.current = saved.history;
      historyIdxRef.current = saved.historyIdx;
      hintMarkers.current = saved.hints;
      setQuestions(saved.questions);
      return;
    }

    const blank = puzzle.questions.map(() => ({
      marks: [...FRESH_MARKS] as Marks,
    }));
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
      const result = validate(puzzle, answers, qs.map((q) => q.marks));
      setValidity(result);

      const isCompleted = result.every((v) => v === "valid");
      saveState(puzzle.id, {
        questions: qs,
        completed: isCompleted,
        history: historyRef.current,
        historyIdx: historyIdxRef.current,
        hints: hintMarkers.current,
      });
      onChanged();
    },
    [puzzle, onChanged],
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
    const next = cloneStates(questionsRef.current);
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
    if (historyRef.current.length <= 1) return;
    const prev = historyRef.current[historyIdxRef.current - 1];
    const curr = historyRef.current[historyIdxRef.current];
    if (prev && describeDiff(prev, curr).qi < 0) return;
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
    if (
      hintRef.current &&
      hintRef.current.step < hintRef.current.steps.length - 1
    ) {
      hintRef.current.step++;
      setHintText(hintRef.current.steps[hintRef.current.step]);
      pushHintMarker(hintRef.current.step + 1);
      return;
    }

    const markSets = questionsRef.current.map((q) => q.marks);
    const result = findHint(puzzle, markSets);
    if (result) {
      hintRef.current = { steps: result.steps, step: 0 };
      setHintText(result.steps[0]);
      pushHintMarker(1);
    } else {
      hintRef.current = null;
      setHintText(null);
    }
  }

  async function handleSharePuzzle() {
    const url = getPuzzleUrl(dateStr, level);
    const ok = await sharePuzzleLink(url, `Refpuzzle Day #${dateStr}`);
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
    const timer = setTimeout(() => setResetPending(false), 3000);
    return () => clearTimeout(timer);
  }, [resetPending]);

  if (questions.length === 0) return null;

  return (
    <div class="puzzle-view">
      {/* Questions */}
      <div
        class={puzzle.difficulty >= 3 ? "questions-grid" : ""}
        style={puzzle.difficulty >= 3 ? { gridTemplateRows: `repeat(${Math.ceil(puzzle.questions.length / 2) * 2}, auto)` } : undefined}
      >
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
          {hintRef.current &&
            hintRef.current.step < hintRef.current.steps.length - 1 && (
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
          {level < 5 && (
            <button class="next-puzzle-btn" onClick={onNextPuzzle}>
              {s.puzzle.nextPuzzle} &rarr;
            </button>
          )}
        </div>
      )}

      {/* Controls */}
      <div class="puzzle-controls">
        <button
          class="toolbar-icon-btn"
          onClick={handleUndo}
          disabled={!canUndo}
          title={s.puzzle.undo}
        >
          &#x21A9;
        </button>
        <button
          class="toolbar-icon-btn"
          onClick={handleRedo}
          disabled={!canRedo}
          title={s.puzzle.redo}
        >
          &#x21AA;
        </button>
        <button class="toolbar-accent-btn" onClick={handleSave}>
          &#x1F4CC; Checkpoint
        </button>
        <span class="controls-spacer"></span>
        <button
          class="toolbar-accent-btn"
          onClick={handleHint}
          title={s.puzzle.hint}
        >
          &#x1F4A1; {s.puzzle.hint}
        </button>
        <span class="split-btn">
          <button class="toolbar-accent-btn" onClick={handleSharePuzzle} title={s.puzzle.sharePuzzle}>
            {s.puzzle.share}
          </button>
          <button class="toolbar-accent-btn split-btn-drop" onClick={handleShareProgress} title={s.puzzle.shareProgress}>
            &#9662;
          </button>
        </span>
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
          hints={hintMarkers.current}
          onJump={handleJumpTo}
        />
      )}

      {/* Toast notification */}
      {toastMessage && (
        <Toast message={toastMessage} onDone={() => setToastMessage(null)} />
      )}
    </div>
  );
}
