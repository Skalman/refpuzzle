import { useState, useEffect, useRef } from "preact/hooks";
import type { TutorialStep } from "../engine/tutorial.ts";
import { LETTERS } from "../engine/types.ts";
import { IconX, IconPlay, IconPause } from "./Icons.tsx";
import type { HighlightInfo } from "./TutorialHighlight.ts";

interface Props {
  steps: TutorialStep[];
  onDismiss: () => void;
  onSetHighlight: (h: HighlightInfo | null) => void;
  onApplyStep: (step: TutorialStep) => void;
  onUnapplyStep: (step: TutorialStep) => void;
  onDone?: () => void;
}

const INTRO_MS = 1400;
const EXPLAIN_MS = 2200;
const FOCUS_MS = 800;
const APPLY_MS = 1200;
const DONE_MS = 4000;

export function TutorialOverlay({
  steps,
  onDismiss,
  onSetHighlight,
  onApplyStep,
  onUnapplyStep,
  onDone,
}: Props) {
  // appliedUpTo: how many deduce steps have been applied to the puzzle (intro steps don't count)
  const [currentIdx, setCurrentIdx] = useState(0);
  const [phase, setPhase] = useState<"show" | "focus" | "applied" | "done">("show");
  const [playing, setPlaying] = useState(true);
  const done = phase === "done";

  const onDismissRef = useRef(onDismiss);
  onDismissRef.current = onDismiss;
  const onApplyRef = useRef(onApplyStep);
  onApplyRef.current = onApplyStep;
  const onUnapplyRef = useRef(onUnapplyStep);
  onUnapplyRef.current = onUnapplyStep;
  const onDoneRef = useRef(onDone);
  onDoneRef.current = onDone;
  const onSetHighlightRef = useRef(onSetHighlight);
  onSetHighlightRef.current = onSetHighlight;

  const step = steps[currentIdx] as TutorialStep | undefined;

  const goNextRef = useRef(goNext);
  goNextRef.current = goNext;
  const goPrevRef = useRef(goPrev);
  goPrevRef.current = goPrev;

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") {
        onDismissRef.current();
        e.preventDefault();
      } else if (e.key === " " || e.key === "Enter") {
        setPlaying((p) => !p);
        e.preventDefault();
      } else if (e.key === "ArrowLeft") {
        goPrevRef.current();
        e.preventDefault();
      } else if (e.key === "ArrowRight") {
        goNextRef.current();
        e.preventDefault();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // Update highlight whenever step or phase changes
  useEffect(() => {
    if (done || !step) {
      onSetHighlightRef.current(null);
      return;
    }
    if (step.kind === "intro") {
      onSetHighlightRef.current({
        qis: step.highlightQis,
        oi: step.highlightOi,
        muteOptions: step.muteOptions,
        noQuestionOutline: step.noQuestionOutline,
      });
    } else {
      if (phase === "focus" || phase === "applied") {
        onSetHighlightRef.current({ qis: [step.questionIndex], oi: step.optionIndex });
      } else {
        onSetHighlightRef.current({ qis: [step.questionIndex] });
      }
    }
  }, [currentIdx, phase, done, step]);

  // Auto-advance timer
  useEffect(() => {
    if (!playing) return undefined;
    let ms: number;
    if (done) ms = DONE_MS;
    else if (!step) ms = DONE_MS;
    else if (step.kind === "intro") ms = step.durationMs ?? INTRO_MS;
    else if (phase === "show") ms = EXPLAIN_MS;
    else if (phase === "focus") ms = FOCUS_MS;
    else ms = APPLY_MS;

    const timer = setTimeout(advance, ms);
    return () => clearTimeout(timer);
    // oxlint-disable-next-line react-hooks/exhaustive-deps
  }, [currentIdx, phase, done, playing]);

  function advance() {
    if (done) {
      onDismissRef.current();
      return;
    }
    if (!step) return;
    if (step.kind === "intro") {
      goToStep(currentIdx + 1);
    } else if (phase === "show") {
      setPhase("focus");
    } else if (phase === "focus") {
      onApplyRef.current(step);
      setPhase("applied");
    } else {
      goToStep(currentIdx + 1);
    }
  }

  function goToStep(idx: number) {
    if (idx >= steps.length) {
      setPhase("done");
      onDoneRef.current?.();
    } else {
      setCurrentIdx(idx);
      setPhase("show");
    }
  }

  function goNext() {
    if (done) return;
    setPlaying(false);
    if (!step) return;
    if (step.kind === "intro") {
      goToStep(currentIdx + 1);
    } else if (phase === "show") {
      setPhase("focus");
    } else {
      if (phase === "focus") {
        onApplyRef.current(step);
      }
      goToStep(currentIdx + 1);
    }
  }

  function goPrev() {
    setPlaying(false);
    // From done: go back to the last step (unapply it)
    if (done) {
      const last = steps[currentIdx];
      if (last?.kind === "deduce") onUnapplyRef.current(last);
      setPhase("show");
      return;
    }
    if (currentIdx <= 0) return;
    // If we applied the current deduce step, unapply it
    if (step?.kind === "deduce" && (phase === "applied" || phase === "focus")) {
      if (phase === "applied") onUnapplyRef.current(step);
      setPhase("show");
      return;
    }
    // Go back — if previous was a deduce step, unapply it
    const prev = steps[currentIdx - 1];
    if (prev?.kind === "deduce") {
      onUnapplyRef.current(prev);
    }
    setCurrentIdx(currentIdx - 1);
    setPhase("show");
  }

  // ── Render: done ──
  if (done) {
    return (
      <div class="tutorial-overlay" onClick={onDismiss}>
        <div class="tutorial-bubble tutorial-done" onClick={(e) => e.stopPropagation()}>
          <button class="tutorial-skip" onClick={onDismiss} aria-label="Dismiss">
            <IconX size="1.2em" />
          </button>
          Solved. Now try it yourself!
        </div>
      </div>
    );
  }

  if (!step) return null;

  // ── Render: intro or deduce ──
  const isIntro = step.kind === "intro";
  const label = isIntro
    ? null
    : step.isForce
      ? `#${step.questionIndex + 1} = ${LETTERS[step.optionIndex]}`
      : `#${step.questionIndex + 1} is not ${LETTERS[step.optionIndex]}`;

  const introSteps = steps.filter((s) => s.kind === "intro");
  const introLogical =
    introSteps.length > 0 && introSteps[0].kind === "intro" ? introSteps[0].introTotal : 0;
  const deduceCount = steps.filter((s) => s.kind === "deduce").length;
  const totalLogical = introLogical + deduceCount;
  const deduceIdx = isIntro
    ? 0
    : steps.slice(0, currentIdx).filter((s) => s.kind === "deduce").length;
  const currentLogical = isIntro ? step.introStep : introLogical + deduceIdx + 1;

  return (
    <div class="tutorial-overlay" onClick={onDismiss}>
      <div class="tutorial-bubble" onClick={(e) => e.stopPropagation()}>
        <button class="tutorial-skip" onClick={onDismiss} aria-label="Skip tutorial">
          <IconX size="1.2em" />
        </button>
        {isIntro ? (
          <>
            <div class="tutorial-action">{step.heading}</div>
            <div class="tutorial-explain">{step.text}</div>
          </>
        ) : (
          <>
            {label && <div class="tutorial-action">{label}</div>}
            {step.explain.length > 0 &&
              (() => {
                const last = step.explain[step.explain.length - 1];
                return (
                  <div class="tutorial-explain">
                    {last.type === "simple" ? last.text : last.header}
                  </div>
                );
              })()}
          </>
        )}
        <div class="tutorial-footer">
          <div class="tutorial-controls">
            <button
              class="tutorial-ctrl-btn"
              onClick={goPrev}
              disabled={currentIdx <= 0}
              aria-label="Previous step"
            >
              ‹
            </button>
            <button
              class="tutorial-ctrl-btn"
              onClick={() => setPlaying((p) => !p)}
              aria-label={playing ? "Pause" : "Play"}
            >
              {playing ? <IconPause size="0.9em" /> : <IconPlay size="0.9em" />}
            </button>
            <button class="tutorial-ctrl-btn" onClick={goNext} aria-label="Next step">
              ›
            </button>
          </div>
          <span class="tutorial-step-count">
            {currentLogical} / {totalLogical}
          </span>
        </div>
        <div class="tutorial-progress">
          <div
            class="tutorial-progress-bar"
            style={{
              width: `${((currentLogical - 1 + (isIntro ? 0.5 : phase === "applied" ? 0.9 : phase === "focus" ? 0.6 : 0.2)) / totalLogical) * 100}%`,
            }}
          />
        </div>
      </div>
    </div>
  );
}
