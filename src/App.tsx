import { useState, useEffect, useCallback, useRef } from "preact/hooks";
import { useForceUpdate } from "./lib/hooks.ts";
import { LocationProvider, Router, Route, useLocation } from "preact-iso";
import { tinykeys } from "tinykeys";
import { PuzzleView } from "./components/PuzzleView.tsx";
import { KeyboardHelp } from "./components/KeyboardHelp.tsx";
import { IconCheck, IconX, IconDot, IconWarning } from "./components/Icons.tsx";
import { exportData, planImport, applyImport } from "./lib/backup.ts";
import type { ImportPlan } from "./lib/backup.ts";
import { joinSync } from "./lib/sync.ts";
// QR components lazy-loaded via dynamic import (no preact dependency in chunks)
import type { Puzzle } from "./engine/types.ts";
import { renderQuestionText, renderOptionLabel, renderClaimLabel } from "./engine/render.ts";
import {
  fetchDaily,
  todayDateStr,
  dayNumber,
  isValidDate,
  dateStrFromOffset,
  puzzleId,
  parseCompactPuzzle,
} from "./puzzles/daily.ts";
import { decodePlaygroundHash } from "./lib/playground.ts";
import { hasState } from "./lib/store.ts";
import { guarded, arrowNavHandler } from "./lib/keyboard.ts";
import { t } from "./i18n/index.ts";
import { replayLogoAnimation } from "./components/Logo.tsx";
import { BackupDialog } from "./components/BackupDialog.tsx";
import { SyncDialog } from "./components/SyncDialog.tsx";
import { ImportPreview } from "./components/ImportPreview.tsx";
import { AppHeader } from "./components/AppHeader.tsx";

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
      <div class={`how-to-play${show ? " how-to-play--first-visit" : ""}`}>
        <h4>{s.help.title}</h4>
        <ol>
          {s.help.howToPlaySteps.map((step, i) => (
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

function downloadBackup(filename: string) {
  const json = exportData();
  const blob = new Blob([json], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}

function useBackupFlow(opts?: { onChanged?: () => void }) {
  const s = t();
  const [showBackup, setShowBackup] = useState(false);
  const [showSync, setShowSync] = useState(false);
  const [importPlan, setImportPlan] = useState<ImportPlan | null>(null);

  function openBackup() {
    setShowBackup(true);
  }
  function closeBackup() {
    setShowBackup(false);
  }

  function handleUploadFile(e: Event) {
    setShowBackup(false);
    const input = e.target;
    if (!(input instanceof HTMLInputElement)) return;
    const file = input.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      try {
        if (typeof reader.result !== "string") return;
        setImportPlan(planImport(reader.result));
      } catch (err) {
        alert(s.backup.uploadFailed(err instanceof Error ? err.message : "unknown error"));
      }
    };
    reader.readAsText(file);
    input.value = "";
  }

  function openSync() {
    setShowBackup(false);
    setShowSync(true);
  }
  function closeSync() {
    setShowSync(false);
  }

  function handleSyncReceive(json: string) {
    setShowSync(false);
    try {
      setImportPlan(planImport(json));
    } catch (err) {
      alert(s.backup.uploadFailed(err instanceof Error ? err.message : "unknown error"));
    }
  }

  function confirmUpload() {
    if (!importPlan) return;
    applyImport(importPlan);
    setImportPlan(null);
    opts?.onChanged?.();
  }

  function cancelUpload() {
    setImportPlan(null);
  }

  return {
    showBackup,
    openBackup,
    closeBackup,
    showSync,
    closeSync,
    importPlan,
    handleUploadFile,
    openSync,
    handleSyncReceive,
    confirmUpload,
    cancelUpload,
  };
}

function BackupDialogs({
  backup,
  exportFilename,
}: {
  backup: ReturnType<typeof useBackupFlow>;
  exportFilename: string;
}) {
  return (
    <>
      {backup.showBackup && (
        <BackupDialog
          onExport={() => downloadBackup(exportFilename)}
          onImport={backup.handleUploadFile}
          onSync={backup.openSync}
          onClose={backup.closeBackup}
        />
      )}
      {backup.showSync && (
        <SyncDialog onImport={backup.handleSyncReceive} onClose={backup.closeSync} />
      )}
      {backup.importPlan && (
        <ImportPreview
          plan={backup.importPlan}
          onConfirm={backup.confirmUpload}
          onCancel={backup.cancelUpload}
        />
      )}
    </>
  );
}

function DailyPage() {
  const dateStr = todayDateStr();
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

  const initialHash = window.location.hash.slice(1) || null;
  const [activeLevel, setActiveLevel] = useState(
    initialLevel && initialLevel >= 1 && initialLevel <= 6 ? initialLevel : 1,
  );

  const activeLevelRef = useRef(activeLevel);
  activeLevelRef.current = activeLevel;
  const showKeyboardHelpRef = useRef(false);

  const tabsRef = useRef<HTMLDivElement>(null);
  const [pendingTutorial, setPendingTutorial] = useState(false);
  const [highlightTab, setHighlightTab] = useState<number | null>(null);
  const [highlightHelp, setHighlightHelp] = useState(false);

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
    // oxlint-disable-next-line typescript/no-unsafe-type-assertion
    const tab = container.children[activeLevel - 1] as HTMLElement | undefined;
    if (!tab) return;
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
        if (activeLevelRef.current > 1) selectLevel(activeLevelRef.current - 1);
      }),
      "]": g(() => {
        if (activeLevelRef.current < 6) selectLevel(activeLevelRef.current + 1);
      }),
      Escape: (ev: KeyboardEvent) => {
        // Priority: dialog handled natively > menu > overlay > pending reset
        const target = ev.target;
        if (target instanceof HTMLElement && target.closest("dialog")) return;
        if (showKeyboardHelpRef.current) {
          showKeyboardHelpRef.current = false;
          setShowKeyboardHelp(false);
        }
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
      showKeyboardHelpRef.current = !showKeyboardHelpRef.current;
      setShowKeyboardHelp(showKeyboardHelpRef.current);
    }
    window.addEventListener("keydown", handleQuestion);

    return () => {
      unsubscribe();
      window.removeEventListener("keydown", handleQuestion);
    };
  }, [selectLevel]);

  useEffect(() => {
    setLoading(true);
    replayLogoAnimation();
    fetchDaily(dateStr).then((data) => {
      setPuzzles(data);
      setLoading(false);
    });
  }, [dateStr]);

  const currentPuzzle = puzzles?.[`${activeLevel}`] ?? null;
  const pid = puzzleId(dateStr, activeLevel);
  if (currentPuzzle) {
    currentPuzzle.id = pid;
  }

  const handleChanged = forcePuzzleUpdate;

  const handleNextLevel = useCallback(() => {
    if (activeLevel < 6) selectLevel(activeLevel + 1);
  }, [activeLevel, selectLevel]);

  const isToday = dateStr === todayDateStr();

  return (
    <>
      <AppHeader
        onKeyboardHelp={() => {
          showKeyboardHelpRef.current = true;
          setShowKeyboardHelp(true);
        }}
        onPrint={puzzles ? () => window.print() : undefined}
        onBackup={backup.openBackup}
      />
      <div class="daily-header">
        {!isToday && (
          <a href="/past" class="back-link">
            &larr; {s.daily.pastPuzzles}
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
        {[1, 2, 3, 4, 5, 6].map((level) => {
          const state = hasState(puzzleId(dateStr, level));
          const { started, completed: solved, stale } = state;
          return (
            <button
              key={level}
              role="tab"
              aria-selected={activeLevel === level}
              tabIndex={activeLevel === level ? 0 : -1}
              class={`difficulty-tab ${activeLevel === level ? "active" : ""} ${solved && !stale ? "tab-solved" : ""} ${stale ? "tab-stale" : ""} ${started ? "tab-started" : ""}${highlightTab === level ? " tutorial-highlight-btn" : ""}`}
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
          onStartTutorial={() => {
            if (activeLevel === 1) {
              setPendingTutorial(true);
            } else {
              setHighlightTab(1);
              setTimeout(() => {
                setHighlightTab(null);
                setPendingTutorial(true);
                selectLevel(1);
              }, 1200);
            }
          }}
          autoStartTutorial={pendingTutorial && activeLevel === 1}
          onTutorialConsumed={() => setPendingTutorial(false)}
          onTutorialDone={() => setHighlightHelp(true)}
        />
      )}

      {showKeyboardHelp && (
        <KeyboardHelp
          onClose={() => {
            setShowKeyboardHelp(false);
            showKeyboardHelpRef.current = false;
          }}
        />
      )}

      <InlineHelp highlight={highlightHelp} />

      {puzzles && (
        <div class="print-only">
          <h1>
            {s.app.title} &mdash; {s.daily.dayLabel(dayNumber(dateStr), dateStr)}
          </h1>
          {[1, 2, 3, 4, 5, 6].map((lvl) => {
            const p = puzzles[`${lvl}`];
            if (!p) return null;
            return (
              <div key={lvl} class="print-puzzle">
                <h2>
                  {s.difficulty[lvl]} ({p.questions.length} {s.puzzleList.questions})
                </h2>
                {p.questions.map((q, qi) => {
                  const labels = q.options.map((opt, oi) =>
                    "claim" in opt
                      ? renderClaimLabel(opt.claim)
                      : renderOptionLabel(q.questionType, opt.value, oi),
                  );
                  return (
                    // oxlint-disable-next-line react/no-array-index-key
                    <div key={qi} class="print-question">
                      <div class="print-question-text">
                        {qi + 1}. {renderQuestionText(q.questionType)}
                      </div>
                      <div
                        class={`print-options ${labels.some((l) => l.length > 12) ? "print-options-long" : ""}`}
                      >
                        {labels.map((label, oi) => (
                          // oxlint-disable-next-line react/no-array-index-key
                          <span key={oi} class="print-option">
                            {String.fromCharCode(65 + oi)}. {label}
                          </span>
                        ))}
                      </div>
                    </div>
                  );
                })}
              </div>
            );
          })}
        </div>
      )}

      <BackupDialogs backup={backup} exportFilename={`refpuzzle-backup-${dateStr}.json`} />
    </>
  );
}

function DayItem({ dateStr, isToday }: { dateStr: string; isToday: boolean }) {
  const s = t();
  const levels = [1, 2, 3, 4, 5, 6].map((l) => {
    const { started, completed, stale } = hasState(puzzleId(dateStr, l));
    return { level: l, started, completed, stale };
  });
  const solved = levels.filter((l) => l.completed && !l.stale);
  const stale = levels.filter((l) => l.stale);
  const started = levels.filter((l) => l.started && !l.completed && !l.stale);
  return (
    <a href={`/${dateStr}/1`} class={`history-item ${isToday ? "history-today" : ""}`}>
      <span class="history-date">
        {isToday ? s.daily.today : dateStr}
        <span class="history-day"> {s.daily.dayNumber(dayNumber(dateStr))}</span>
      </span>
      <span class="history-progress">
        {solved.length === 6 ? (
          s.daily.allSolved
        ) : solved.length > 0 || stale.length > 0 || started.length > 0 ? (
          <>
            {solved.length > 0 && (
              <span>
                <IconCheck size="0.9em" strokeWidth={3} class="icon-correct" />{" "}
                {solved.map((l) => s.difficulty[l.level]).join(", ")}
              </span>
            )}
            {stale.length > 0 && (
              <span>
                {" "}
                <IconWarning size="0.9em" class="icon-stale" />{" "}
                {stale.map((l) => s.difficulty[l.level]).join(", ")}
              </span>
            )}
            {(solved.length > 0 || stale.length > 0) && started.length > 0 && "  "}
            {started.length > 0 && (
              <span>
                <IconDot size="0.9em" class="icon-hint" />{" "}
                {started.map((l) => s.difficulty[l.level]).join(", ")}
              </span>
            )}
          </>
        ) : (
          s.daily.notStarted
        )}
      </span>
    </a>
  );
}

function PastPuzzlesPage() {
  const s = t();
  const backup = useBackupFlow();
  const today = todayDateStr();
  const currentMonth = today.slice(0, 7);

  const allDates: string[] = [];
  for (let i = 0; ; i++) {
    const d = dateStrFromOffset(i);
    if (!isValidDate(d)) break;
    allDates.push(d);
  }

  const months = new Map<string, string[]>();
  for (const d of allDates) {
    const month = d.slice(0, 7);
    const list = months.get(month) ?? [];
    list.push(d);
    months.set(month, list);
  }

  const [expanded, setExpanded] = useState<Set<string>>(() => new Set([currentMonth]));

  function toggleMonth(month: string) {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(month)) next.delete(month);
      else next.add(month);
      return next;
    });
  }

  function formatMonth(ym: string): string {
    const [y, m] = ym.split("-");
    const date = new Date(Number(y), Number(m) - 1);
    return date.toLocaleString(undefined, { month: "long", year: "numeric" });
  }

  return (
    <>
      <AppHeader onBackup={backup.openBackup} />

      <div class="history-page">
        <h2>{s.daily.pastPuzzles}</h2>
        {[...months.entries()].map(([month, dates]) => {
          const isOpen = expanded.has(month);
          return (
            <div key={month} class="history-month">
              <button class="history-month-header" onClick={() => toggleMonth(month)}>
                <span>{formatMonth(month)}</span>
                <span class={`history-month-arrow ${isOpen ? "open" : ""}`}>&#9662;</span>
              </button>
              {isOpen && (
                <div class="history-list">
                  {dates.map((dateStr) => (
                    <DayItem key={dateStr} dateStr={dateStr} isToday={dateStr === today} />
                  ))}
                </div>
              )}
            </div>
          );
        })}
      </div>

      <BackupDialogs backup={backup} exportFilename="refpuzzle-backup.json" />
    </>
  );
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
        <Router>
          <Route path="/" component={DailyPage} />
          <Route path="/past" component={PastPuzzlesPage} />
          <Route path="/sync" component={SyncRoute} />
          <Route path="/playground" component={PlaygroundRoute} />
          <Route path="/:date/:level" component={DayRoute} />
          <Route default component={NotFound} />
        </Router>
      </div>
    </LocationProvider>
  );
}
