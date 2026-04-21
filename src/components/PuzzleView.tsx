import { useState, useEffect, useCallback, useRef } from "preact/hooks";
import { useRoute } from "preact-iso";
import type { AnswerLetter, Marks } from "../engine/types.ts";
import { LETTERS } from "../engine/types.ts";
import { allPuzzles } from "../puzzles/index.ts";
import { validate } from "../engine/validate.ts";
import { findHint } from "../engine/hints.ts";
import { loadState, saveState } from "../lib/store.ts";
import type { QuestionState } from "../lib/store.ts";
import { encodeState, decodeState, getShareUrl } from "../lib/share.ts";
import { t } from "../i18n/index.ts";
import { QuestionRow } from "./QuestionRow.tsx";
import { Logo } from "./Logo.tsx";

const FRESH_MARKS: Marks = ["unmarked", "unmarked", "unmarked", "unmarked", "unmarked"];

function StateDisplay({
  questionStates,
  puzzleId,
}: {
  questionStates: QuestionState[];
  puzzleId: string;
}) {
  const markSets = questionStates.map((q) => q.marks);
  const stateStr = encodeState(markSets);
  const url = getShareUrl(puzzleId, markSets);

  function handleCopy() {
    navigator.clipboard.writeText(url);
  }

  return (
    <div class="state-display">
      <code class="state-code">{stateStr}</code>
      <button class="state-copy" onClick={handleCopy} title="Copy link">
        copy link
      </button>
    </div>
  );
}

function cloneStates(qs: QuestionState[]): QuestionState[] {
  return qs.map((q) => ({ marks: [...q.marks] as Marks }));
}

interface MoveInfo {
  label: string;
  qi: number;
  oi: number;
}

function describeDiff(prev: QuestionState[], next: QuestionState[]): MoveInfo {
  // Prioritize: correct > incorrect > undo
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
    // No diff at all — this is a checkpoint
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

export function PuzzleView() {
  const { params } = useRoute();
  const puzzle = allPuzzles.find((p) => p.id === params.id);
  const s = t();

  const [questions, setQuestions] = useState<QuestionState[]>([]);
  const [validity, setValidity] = useState<("neutral" | "valid" | "invalid")[]>([]);
  const [hintText, setHintText] = useState<string | null>(null);
  const hintRef = useRef<{ steps: string[]; step: number } | null>(null);

  const historyRef = useRef<QuestionState[][]>([]);
  const historyIdxRef = useRef(-1);
  const [_historyVersion, setHistoryVersion] = useState(0);

  function pushHistory(qs: QuestionState[]) {
    const h = historyRef.current;
    const idx = historyIdxRef.current;
    // Truncate any future entries
    historyRef.current = h.slice(0, idx + 1);

    // If this move touches the same Q+option as the last move, replace it
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
    if (!puzzle) return;
    const n = puzzle.questions.length;
    let initial: QuestionState[] | null = null;

    const hash = window.location.hash.slice(1);
    if (hash) {
      const decoded = decodeState(hash);
      if (decoded && decoded.length === n) {
        initial = decoded.map((marks) => ({ marks }));
        history.replaceState(null, "", window.location.pathname);
      }
    }

    if (!initial) {
      const saved = loadState(puzzle.id);
      initial = saved
        ? saved.questions
        : puzzle.questions.map(() => ({ marks: [...FRESH_MARKS] as Marks }));
    }

    setQuestions(initial);
    historyRef.current = [cloneStates(initial)];
    historyIdxRef.current = 0;
  }, [puzzle]);

  const revalidate = useCallback(
    (qs: QuestionState[]) => {
      if (!puzzle) return;
      const answers: (AnswerLetter | null)[] = qs.map((q) => {
        const idx = q.marks.indexOf("correct");
        return idx >= 0 ? LETTERS[idx] : null;
      });
      const result = validate(puzzle, answers);
      setValidity(result);

      const completed = result.every((v) => v === "valid");
      saveState(puzzle.id, { questions: qs, completed });
    },
    [puzzle],
  );

  useEffect(() => {
    if (questions.length > 0) revalidate(questions);
  }, [questions, revalidate]);

  if (!puzzle) {
    return (
      <div class="not-found">
        <h1>?</h1>
        <p>Puzzle not found</p>
        <a href="/">Back to puzzles</a>
      </div>
    );
  }

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

  function handleCheckpoint() {
    pushHistory(cloneStates(questions));
  }

  function handleReset() {
    if (!puzzle) return;
    const fresh = puzzle.questions.map(() => ({
      marks: [...FRESH_MARKS] as Marks,
    }));
    applyChange(fresh);
  }

  function handleHint() {
    if (!puzzle) return;

    // If already showing a hint, advance to next step
    if (hintRef.current && hintRef.current.step < hintRef.current.steps.length - 1) {
      hintRef.current.step++;
      setHintText(hintRef.current.steps[hintRef.current.step]);
      return;
    }

    // Otherwise get a new hint
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

  return (
    <>
      <header class="app-header">
        <h1>
          <a href="/"><Logo />{s.app.title}</a>
        </h1>
      </header>
      <div class="puzzle-header">
        <h2>{puzzle.title}</h2>
        <a href="/">{s.puzzle.back}</a>
      </div>
      <div class={puzzle.difficulty >= 4 ? "questions-grid" : ""}>
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
      {completed && <div class="puzzle-complete">{s.puzzle.solved}</div>}
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
      <div class="puzzle-controls">
        <div class="puzzle-actions">
          <button onClick={handleUndo} disabled={!canUndo}>
            {s.puzzle.undo}
          </button>
          <button onClick={handleRedo} disabled={!canRedo}>
            {s.puzzle.redo}
          </button>
          <button onClick={handleCheckpoint}>Checkpoint</button>
          <button onClick={handleHint}>{s.puzzle.hint}</button>
          <button onClick={handleReset}>{s.puzzle.reset}</button>
        </div>
        <HistoryStrip
          history={historyRef.current}
          currentIdx={historyIdxRef.current}
          onJump={handleJumpTo}
        />
        <StateDisplay questionStates={questions} puzzleId={puzzle.id} />
      </div>
    </>
  );
}
