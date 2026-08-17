import { useState, useEffect, useCallback, useRef } from "preact/hooks";
import { useForceUpdate, useRevalidated } from "./lib/hooks.ts";
import { LocationProvider, Router, Route, useLocation } from "preact-iso";
import { tinykeys } from "tinykeys";
import { PuzzleView } from "./components/PuzzleView.tsx";
import { KeyboardHelp } from "./components/KeyboardHelp.tsx";
import { IconCheck, IconX, IconDot, IconWarning } from "./components/Icons.tsx";
import { planImport, applyImport } from "./lib/backup.ts";
import type { ImportPlan } from "./lib/backup.ts";
import { joinSync } from "./lib/sync.ts";
// QR components lazy-loaded via dynamic import (no preact dependency in chunks)
import type { Puzzle } from "./engine/types.ts";
import { LETTERS } from "./engine/types.ts";
import {
  LEVELS,
  fetchDaily,
  dayNumber,
  isValidDate,
  puzzleId,
  parseCompactPuzzle,
} from "./puzzles/daily.ts";
import { dayStates, resumeLevel } from "./puzzles/progress.ts";
import { useToday } from "./lib/today.ts";
import { classNames } from "./lib/classNames.ts";
import { decodePlaygroundHash } from "./lib/playground.ts";
import { hasState } from "./lib/store.ts";
import { guarded, arrowNavHandler } from "./lib/keyboard.ts";
import { pointerKind } from "./lib/pointer.ts";
import { t } from "./i18n/index.ts";
import { replayLogoAnimation } from "./components/Logo.tsx";
import { ImportPreview } from "./components/ImportPreview.tsx";
import { AppHeader } from "./components/AppHeader.tsx";
import { ArchivePage } from "./components/ArchivePage.tsx";
import { useBackupFlow, BackupDialogs } from "./components/BackupFlow.tsx";
import { ErrorOverlay } from "./components/ErrorOverlay.tsx";

if (new URLSearchParams(window.location.search).has("debug")) {
  sessionStorage.setItem("debug", "1");
}

function InlineHelp({ highlight }: { highlight?: boolean }) {
  const s = t();
  const [firstVisit, setFirstVisit] = useState(() => {
    try {
      return !localStorage.getItem("refpuzzle:onboarded");
    } catch {
      return false;
    }
  });

  useEffect(() => {
    if (!firstVisit) return undefined;
    try {
      localStorage.setItem("refpuzzle:onboarded", "1");
    } catch {
      // ignore
    }
    const timer = setTimeout(() => setFirstVisit(false), 15000);
    return () => clearTimeout(timer);
  }, [firstVisit]);

  const show = highlight || firstVisit;

  return (
    <div class="inline-help">
      <div class={classNames("how-to-play", show && "how-to-play--first-visit")}>
        <h4>{s.help.title}</h4>
        <p class="how-to-goal">{s.help.goal}</p>
        <ol>
          {s.help.howToPlaySteps(pointerKind()).map((step, i) => (
            <li key={step}>
              {step}
              {i === 0 && (
                <>
                  {" "}
                  <span class="nowrap">
                    (<IconX size="0.9em" strokeWidth={3} class="icon-incorrect" />)
                  </span>
                </>
              )}
              {i === 1 && (
                <>
                  {" "}
                  <span class="nowrap">
                    (<IconCheck size="0.9em" strokeWidth={3} class="icon-correct" />)
                  </span>
                </>
              )}
            </li>
          ))}
        </ol>
      </div>
      <h4>{s.help.whatIs}</h4>
      {s.help.descriptionParagraphs.map((p) => (
        <p key={p}>{p}</p>
      ))}
    </div>
  );
}

function DailyPage() {
  const dateStr = useToday();
  return <DayView dateStr={dateStr} />;
}

function DayView({ dateStr, initialLevel }: { dateStr: string; initialLevel?: number }) {
  const s = t();
  const { route } = useLocation();
  const [showKeyboardHelp, setShowKeyboardHelp] = useState(false);
  const [puzzles, setPuzzles] = useState<Record<string, Puzzle> | null>(null);
  const [loading, setLoading] = useState(true);
  const forcePuzzleUpdate = useForceUpdate();
  const backup = useBackupFlow({ onChanged: forcePuzzleUpdate });
  useRevalidated();

  const initialHash = window.location.hash.slice(1) || null;
  // A level in the path is the one asked for; otherwise the day opens wherever
  // it was left. Only the mount decides — a rollover past midnight keeps the
  // level on screen rather than moving it out from under the solver.
  const [activeLevel, setActiveLevel] = useState(() =>
    initialLevel && LEVELS.includes(initialLevel) ? initialLevel : resumeLevel(dayStates(dateStr)),
  );

  const tabsRef = useRef<HTMLDivElement>(null);

  const selectLevel = useCallback(
    (level: number) => {
      setActiveLevel(level);
      route(`/${dateStr}/${level}`, true);
      replayLogoAnimation();
    },
    [dateStr, route],
  );

  const activeTabState = hasState(puzzleId(dateStr, activeLevel));
  const activeTabIcon = activeTabState.stale
    ? "stale"
    : activeTabState.completed
      ? "solved"
      : activeTabState.started
        ? "started"
        : "";

  useEffect(() => {
    const container = tabsRef.current;
    if (!container) return;
    const tab = container.children[activeLevel - 1];
    if (!(tab instanceof HTMLElement)) return;
    // Center the tab horizontally without affecting vertical scroll (scrollIntoView would
    // also scroll the page vertically when the tab isn't fully in view).
    const tabRect = tab.getBoundingClientRect();
    const containerRect = container.getBoundingClientRect();
    const delta = tabRect.left + tabRect.width / 2 - (containerRect.left + containerRect.width / 2);
    container.scrollTo({ left: container.scrollLeft + delta, behavior: "smooth" });
  }, [activeLevel, activeTabIcon]);

  // Page-level keyboard shortcuts
  useEffect(() => {
    const g = guarded;
    const unsubscribe = tinykeys(window, {
      "[": g(() => {
        if (activeLevel > 1) selectLevel(activeLevel - 1);
      }),
      "]": g(() => {
        if (activeLevel < LEVELS.length) selectLevel(activeLevel + 1);
      }),
      Escape: (ev: KeyboardEvent) => {
        // Priority: dialog handled natively > menu > overlay
        const target = ev.target;
        if (target instanceof HTMLElement && target.closest("dialog")) return;
        setShowKeyboardHelp(false);
      },
    });

    // "?" bypasses tinykeys — tinykeys rejects shiftKey when Shift isn't in
    // the binding, and "?" inherently requires Shift on most layouts. Matching
    // event.key directly is layout-independent.
    function handleQuestion(ev: KeyboardEvent) {
      if (ev.key !== "?") return;
      const el = ev.target;
      if (
        el instanceof HTMLElement &&
        (el.closest("dialog") ||
          el.tagName === "INPUT" ||
          el.tagName === "TEXTAREA" ||
          el.tagName === "SELECT")
      )
        return;
      setShowKeyboardHelp((shown) => !shown);
    }
    window.addEventListener("keydown", handleQuestion);

    return () => {
      unsubscribe();
      window.removeEventListener("keydown", handleQuestion);
    };
  }, [activeLevel, selectLevel]);

  // A fast date change can resolve out of order, so only the newest fetch wins.
  useEffect(() => {
    let canceled = false;
    setLoading(true);
    replayLogoAnimation();
    void fetchDaily(dateStr).then((data) => {
      if (canceled) return;
      setPuzzles(data);
      setLoading(false);
    });
    return () => {
      canceled = true;
    };
  }, [dateStr]);

  const currentPuzzle = puzzles?.[`${activeLevel}`] ?? null;
  const pid = puzzleId(dateStr, activeLevel);

  const handleChanged = forcePuzzleUpdate;

  const handleNextLevel = useCallback(() => {
    if (activeLevel < LEVELS.length) selectLevel(activeLevel + 1);
  }, [activeLevel, selectLevel]);

  const today = useToday();
  const isToday = dateStr === today;

  return (
    <>
      <AppHeader
        onKeyboardHelp={() => setShowKeyboardHelp(true)}
        onPrint={puzzles ? () => window.print() : undefined}
        onBackup={backup.openBackup}
      />
      <div class="daily-header">
        {!isToday && (
          <a href="/archive" class="back-link">
            &larr; {s.daily.archive}
          </a>
        )}
        <span class="daily-date">{s.daily.dayLabel(dayNumber(dateStr), dateStr)}</span>
      </div>

      <div
        ref={tabsRef}
        class="difficulty-tabs"
        role="tablist"
        onKeyDown={arrowNavHandler(".difficulty-tab")}
      >
        {LEVELS.map((level) => {
          const { started, completed: solved, stale } = hasState(puzzleId(dateStr, level));
          return (
            <button
              key={level}
              role="tab"
              aria-selected={activeLevel === level}
              tabIndex={activeLevel === level ? 0 : -1}
              class={classNames(
                "difficulty-tab",
                activeLevel === level && "active",
                solved && !stale && "tab-solved",
                stale && "tab-stale",
                started && "tab-started",
              )}
              onClick={() => selectLevel(level)}
            >
              {solved && !stale && (
                <span class="tab-check">
                  <IconCheck size="0.9em" />{" "}
                </span>
              )}
              {stale && (
                <span class="tab-stale-icon">
                  <IconWarning size="0.9em" />{" "}
                </span>
              )}
              {started && !solved && !stale && (
                <span class="tab-started-dot">
                  <IconDot size="0.9em" />{" "}
                </span>
              )}
              <span class="tab-label">{s.difficulty[level]}</span>
            </button>
          );
        })}
      </div>

      {loading && (
        <div class="loading">
          <span class="spinner" />
        </div>
      )}

      {!loading && !currentPuzzle && <div class="loading">{s.app.noPuzzle}</div>}

      {!loading && currentPuzzle && (
        <PuzzleView
          key={pid}
          puzzle={currentPuzzle}
          dateStr={dateStr}
          level={activeLevel}
          initialHash={activeLevel === initialLevel ? initialHash : null}
          onNextPuzzle={handleNextLevel}
          onChanged={handleChanged}
        />
      )}

      {showKeyboardHelp && <KeyboardHelp onClose={() => setShowKeyboardHelp(false)} />}

      <InlineHelp />

      {puzzles && (
        <div class="print-only">
          <h1>
            {s.app.title} &mdash; {s.daily.dayLabel(dayNumber(dateStr), dateStr)}
          </h1>
          {LEVELS.map((level) => {
            const p = puzzles[`${level}`];
            if (!p) return null;
            return (
              <div key={level} class="print-puzzle">
                <h2>
                  {s.difficulty[level]} ({p.questions.length} {s.puzzleList.questions})
                </h2>
                {p.questions.map((q, qi) => (
                  <div key={q.text} class="print-question">
                    <div class="print-question-text">
                      {qi + 1}. {q.text}
                    </div>
                    <div
                      class={classNames(
                        "print-options",
                        q.options.some((l) => l.length > 12) && "print-options-long",
                      )}
                    >
                      {q.options.map((label, oi) => (
                        <span key={LETTERS[oi]} class="print-option">
                          {LETTERS[oi]}. {label}
                        </span>
                      ))}
                    </div>
                  </div>
                ))}
              </div>
            );
          })}
        </div>
      )}

      <BackupDialogs backup={backup} exportFilename={`refpuzzle-backup-${dateStr}.json`} />
    </>
  );
}

/** The archive's old slug; kept so bookmarks and shared links survive. */
function ArchiveRedirect() {
  const { route } = useLocation();
  useEffect(() => {
    route("/archive", true);
  }, [route]);
  return null;
}

function DayRoute() {
  const s = t();
  const loc = useLocation();
  const parts = loc.path.split("/").filter(Boolean);
  const dateStr = parts[0] ?? "";
  const level = Number(parts[1]) || undefined;
  if (!dateStr || !isValidDate(dateStr)) {
    return (
      <div class="not-found">
        <h1>{s.notFound.noPuzzle}</h1>
        <p>{s.app.noPuzzle}</p>
        <a href="/">{s.notFound.backToToday}</a>
      </div>
    );
  }
  return <DayView dateStr={dateStr} initialLevel={level} />;
}

function SyncRoute() {
  const s = t();
  const code = window.location.hash.slice(1);
  const [status, setStatus] = useState<"joining" | "done" | "error">("joining");
  const [importPlan, setImportPlan] = useState<ImportPlan | null>(null);

  useEffect(() => {
    if (!/^\d{6}$/.test(code)) {
      setStatus("error");
      return;
    }
    joinSync(code)
      .then((json) => {
        try {
          setImportPlan(planImport(json));
          setStatus("done");
        } catch {
          setStatus("error");
        }
      })
      .catch(() => setStatus("error"));
  }, [code]);

  return (
    <div class="not-found">
      {status === "joining" && (
        <div class="loading">
          <span class="spinner" />
        </div>
      )}
      {status === "error" && (
        <>
          <h1>{s.sync.expired}</h1>
          <a href="/">{s.notFound.backToPuzzles}</a>
        </>
      )}
      {importPlan && (
        <ImportPreview
          plan={importPlan}
          onConfirm={() => {
            applyImport(importPlan);
            setImportPlan(null);
            window.location.href = "/";
          }}
          onCancel={() => {
            window.location.href = "/";
          }}
        />
      )}
    </div>
  );
}

function PlaygroundRoute() {
  const hash = window.location.hash.slice(1);
  type State =
    | { status: "loading" }
    | { status: "error" }
    | { status: "ready"; puzzle: Puzzle; stateHash: string | null };
  const [state, setState] = useState<State>({ status: "loading" });

  useEffect(() => {
    if (!hash) {
      setState({ status: "error" });
      return;
    }
    decodePlaygroundHash(hash)
      .then((decoded) => {
        if (!decoded) {
          setState({ status: "error" });
          return;
        }
        setState({
          status: "ready",
          puzzle: parseCompactPuzzle(decoded.compact),
          stateHash: decoded.stateHash,
        });
      })
      .catch(() => setState({ status: "error" }));
  }, [hash]);

  if (state.status === "loading")
    return (
      <div class="loading">
        <span class="spinner" />
      </div>
    );
  if (state.status === "error") return <div class="loading">Invalid puzzle hash.</div>;
  return (
    <PuzzleView
      key={hash}
      puzzle={state.puzzle}
      dateStr="playground"
      level={1}
      initialHash={state.stateHash}
      ephemeral
      onNextPuzzle={() => {}}
      onChanged={() => {}}
    />
  );
}

function NotFound() {
  const s = t();
  return (
    <div class="not-found">
      <h1>{s.notFound.title}</h1>
      <p>{s.notFound.pageNotFound}</p>
      <a href="/">{s.notFound.backToPuzzles}</a>
    </div>
  );
}

export function App() {
  return (
    <LocationProvider>
      <div class="page">
        <ErrorOverlay />
        <Router>
          <Route path="/" component={DailyPage} />
          <Route path="/archive" component={ArchivePage} />
          <Route path="/past" component={ArchiveRedirect} />
          <Route path="/sync" component={SyncRoute} />
          <Route path="/playground" component={PlaygroundRoute} />
          <Route path="/:date/:level" component={DayRoute} />
          <Route default component={NotFound} />
        </Router>
      </div>
    </LocationProvider>
  );
}
