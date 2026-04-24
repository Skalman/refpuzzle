import { useState, useEffect, useCallback, useRef } from "preact/hooks";
import { LocationProvider, Router, Route, useLocation } from "preact-iso";
import { PuzzleView } from "./components/PuzzleView.tsx";
import {
  IconCalendar,
  IconHelp,
  IconMoon,
  IconSun,
  IconSunMoon,
  IconCheck,
  IconX,
} from "./components/Icons.tsx";
import { exportData, planImport, applyImport } from "./lib/backup.ts";
import type { ImportPlan, ImportAction } from "./lib/backup.ts";
import type { Puzzle } from "./engine/types.ts";
import {
  fetchDaily,
  todayDateStr,
  dayNumber,
  isValidDate,
  dateStrFromOffset,
  puzzleId,
} from "./puzzles/daily.ts";
import { hasState } from "./lib/store.ts";
import { t } from "./i18n/index.ts";
import { Logo } from "./components/Logo.tsx";

function useTheme() {
  const [mode, setMode] = useState(
    () => document.documentElement.getAttribute("data-theme") ?? "auto",
  );

  const cycle = useCallback(() => {
    const html = document.documentElement;
    const current = html.getAttribute("data-theme");
    if (current === "dark") {
      html.setAttribute("data-theme", "light");
      localStorage.setItem("refpuzzle:theme", "light");
      setMode("light");
    } else if (current === "light") {
      html.removeAttribute("data-theme");
      localStorage.removeItem("refpuzzle:theme");
      setMode("auto");
    } else {
      html.setAttribute("data-theme", "dark");
      localStorage.setItem("refpuzzle:theme", "dark");
      setMode("dark");
    }
  }, []);

  const icon = mode === "dark" ? <IconMoon /> : mode === "light" ? <IconSun /> : <IconSunMoon />;

  return { mode, cycle, icon };
}

function OnboardingBanner() {
  const s = t();
  const [visible, setVisible] = useState(() => {
    try {
      return !localStorage.getItem("refpuzzle:onboarded");
    } catch {
      return true;
    }
  });

  if (!visible) return null;

  function dismiss() {
    setVisible(false);
    try {
      localStorage.setItem("refpuzzle:onboarded", "1");
    } catch {
      // ignore
    }
  }

  return (
    <div class="onboarding-banner">
      <div class="onboarding-content">
        <strong>{s.onboarding.welcome}</strong>
        <ul>
          <li>
            {s.onboarding.step1} (<IconX size="0.9em" strokeWidth={3} class="icon-incorrect" />)
          </li>
          <li>
            {s.onboarding.step2} (<IconCheck size="0.9em" strokeWidth={3} class="icon-correct" />)
          </li>
          <li>{s.onboarding.step3}</li>
        </ul>
      </div>
      <button class="onboarding-dismiss" onClick={dismiss}>
        {s.onboarding.gotIt}
      </button>
    </div>
  );
}

function HelpPanel({ onClose }: { onClose: () => void }) {
  const s = t();
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    ref.current?.showModal();
  }, []);
  return (
    <dialog
      ref={ref}
      class="help-panel"
      onClose={onClose}
      onClick={(e) => {
        if (e.target === ref.current) onClose();
      }}
    >
      <div class="help-panel-header">
        <h3>{s.help.title}</h3>
        <button class="help-close" onClick={onClose} aria-label="Close">
          &times;
        </button>
      </div>
      <ol>
        {s.help.howToPlaySteps.map((step, i) => (
          <li key={step}>
            {step}
            {i === 0 && (
              <>
                {" "}
                (<IconX size="0.9em" strokeWidth={3} class="icon-incorrect" />)
              </>
            )}
            {i === 1 && (
              <>
                {" "}
                (<IconCheck size="0.9em" strokeWidth={3} class="icon-correct" />)
              </>
            )}
          </li>
        ))}
      </ol>
      <h4>{s.help.howToSolve}</h4>
      <ol>
        {s.help.howToSolveSteps.map((step) => (
          <li key={step}>{step}</li>
        ))}
      </ol>
      <h4>{s.help.whatIs}</h4>
      <p>{s.help.description}</p>
      <p class="help-credit">
        Inspired by{" "}
        <a href="https://www.logiquiz.com/" target="_blank" rel="noopener noreferrer">
          Logiquiz
        </a>
      </p>
    </dialog>
  );
}

function AppHeader({
  onHelp,
  onPrint,
  onExport,
  onImport,
}: {
  onHelp: () => void;
  onPrint?: () => void;
  onExport: () => void;
  onImport: (e: Event) => void;
}) {
  const s = t();
  const theme = useTheme();
  const [moreMenu, setMoreMenu] = useState(false);

  useEffect(() => {
    if (!moreMenu) return undefined;
    const close = () => setMoreMenu(false);
    document.addEventListener("click", close);
    return () => document.removeEventListener("click", close);
  }, [moreMenu]);

  return (
    <header class="app-header">
      <h1>
        <a href="/" class="app-title-link">
          <Logo />
          <span class="app-title">
            <span class="app-title-ref">Ref</span>puzzle
          </span>
        </a>
      </h1>
      <div class="header-actions">
        <a href="/past" class="header-btn hide-mobile" aria-label={s.daily.pastPuzzles}>
          <IconCalendar /> {s.daily.pastPuzzles}
        </a>
        <button class="header-btn hide-mobile" onClick={onHelp} aria-label="Help">
          <IconHelp /> {s.help.title}
        </button>
        <button class="header-btn hide-mobile" onClick={theme.cycle} aria-label="Toggle theme">
          {theme.icon} Theme
        </button>
        <span class="more-menu-wrapper">
          <button
            class="header-btn more-btn"
            onClick={(e) => {
              e.stopPropagation();
              setMoreMenu((v) => !v);
            }}
            aria-label="More"
          >
            ⋯
          </button>
          {moreMenu && (
            <div class="more-menu">
              <a href="/past" class="more-menu-item show-mobile" onClick={() => setMoreMenu(false)}>
                {s.daily.pastPuzzles}
              </a>
              <button
                class="more-menu-item show-mobile"
                onClick={() => {
                  setMoreMenu(false);
                  onHelp();
                }}
              >
                {s.help.title}
              </button>
              <button
                class="more-menu-item show-mobile"
                onClick={(e) => {
                  e.stopPropagation();
                  theme.cycle();
                }}
              >
                {theme.icon} Theme
              </button>
              <hr class="more-menu-divider show-mobile" />
              {onPrint && (
                <button
                  class="more-menu-item"
                  onClick={() => {
                    setMoreMenu(false);
                    onPrint();
                  }}
                >
                  {s.daily.printAll}
                </button>
              )}
              <button
                class="more-menu-item"
                onClick={() => {
                  setMoreMenu(false);
                  onExport();
                }}
              >
                {s.backup.exportData}
              </button>
              <label class="more-menu-item">
                {s.backup.importData}
                <input
                  type="file"
                  accept=".json"
                  class="file-input"
                  onChange={(e) => {
                    setMoreMenu(false);
                    onImport(e);
                  }}
                />
              </label>
            </div>
          )}
        </span>
      </div>
    </header>
  );
}

function DailyPage() {
  const dateStr = todayDateStr();
  return <DayView dateStr={dateStr} />;
}

function DayView({ dateStr }: { dateStr: string }) {
  const s = t();
  const [showHelp, setShowHelp] = useState(false);
  const [puzzles, setPuzzles] = useState<Record<string, Puzzle> | null>(null);
  const [loading, setLoading] = useState(true);
  const [_puzzleVersion, setPuzzleVersion] = useState(0);
  const [importPlan, setImportPlan] = useState<ImportPlan | null>(null);

  const params = new URLSearchParams(window.location.search);
  const hashLevel = Number(params.get("l")) || 0;
  const initialHash = window.location.hash.slice(1) || null;
  const [activeLevel, setActiveLevel] = useState(hashLevel >= 1 && hashLevel <= 5 ? hashLevel : 1);

  const selectLevel = useCallback(
    (level: number) => {
      setActiveLevel(level);
      window.history.replaceState(null, "", `/day/${dateStr}?l=${level}`);
    },
    [dateStr],
  );

  useEffect(() => {
    setLoading(true);
    fetchDaily(dateStr).then((data) => {
      setPuzzles(data);
      setLoading(false);
    });
  }, [dateStr]);

  const currentPuzzle = puzzles?.[`level-${activeLevel}`] ?? null;
  const pid = puzzleId(dateStr, activeLevel);
  if (currentPuzzle) {
    currentPuzzle.id = pid;
  }

  const handleChanged = useCallback(() => {
    setPuzzleVersion((v) => v + 1);
  }, []);

  const handleNextLevel = useCallback(() => {
    if (activeLevel < 5) selectLevel(activeLevel + 1);
  }, [activeLevel, selectLevel]);

  function handleExport() {
    const json = exportData();
    const blob = new Blob([json], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `refpuzzle-backup-${dateStr}.json`;
    a.click();
    URL.revokeObjectURL(url);
  }

  function handleImport(e: Event) {
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
        alert(s.backup.importFailed(err instanceof Error ? err.message : "unknown error"));
      }
    };
    reader.readAsText(file);
    input.value = "";
  }

  function handleConfirmImport() {
    if (!importPlan) return;
    applyImport(importPlan);
    setImportPlan(null);
    setPuzzleVersion((v) => v + 1);
  }

  const isToday = dateStr === todayDateStr();

  return (
    <>
      <AppHeader
        onHelp={() => setShowHelp(true)}
        onPrint={puzzles ? () => window.print() : undefined}
        onExport={handleExport}
        onImport={handleImport}
      />
      {isToday && <OnboardingBanner />}

      <div class="daily-header">
        {!isToday && (
          <a href="/past" class="back-link">
            &larr; {s.daily.pastPuzzles}
          </a>
        )}
        <span class="daily-date">{s.daily.dayLabel(dayNumber(dateStr), dateStr)}</span>
      </div>

      <div class="difficulty-tabs">
        {[1, 2, 3, 4, 5].map((level) => {
          const state = hasState(puzzleId(dateStr, level));
          const { started, completed: solved } = state;
          return (
            <button
              key={level}
              class={`difficulty-tab ${activeLevel === level ? "active" : ""} ${solved ? "tab-solved" : ""} ${started ? "tab-started" : ""}`}
              onClick={() => selectLevel(level)}
            >
              {solved && (
                <span class="tab-check">
                  <IconCheck size="0.9em" strokeWidth={3} />{" "}
                </span>
              )}
              {started && !solved && <span class="tab-started-dot">&#8226; </span>}
              <span class="tab-label">{s.difficulty[level]}</span>
            </button>
          );
        })}
      </div>

      {loading && <div class="loading">{s.app.loading}</div>}

      {!loading && !currentPuzzle && <div class="loading">{s.app.noPuzzle}</div>}

      {!loading && currentPuzzle && (
        <PuzzleView
          key={pid}
          puzzle={currentPuzzle}
          dateStr={dateStr}
          level={activeLevel}
          initialHash={hashLevel === activeLevel ? initialHash : null}
          onNextPuzzle={handleNextLevel}
          onChanged={handleChanged}
        />
      )}

      {showHelp && <HelpPanel onClose={() => setShowHelp(false)} />}

      <div class="inline-help">
        <h4>{s.help.whatIs}</h4>
        <p>{s.help.description}</p>
        <h4>{s.help.title}</h4>
        <ol>
          {s.help.howToPlaySteps.map((step) => (
            <li key={step}>{step}</li>
          ))}
        </ol>
      </div>

      {puzzles && (
        <div class="print-only">
          <h1>
            Refpuzzle &mdash; Day #{dayNumber(dateStr)} &mdash; {dateStr}
          </h1>
          {[1, 2, 3, 4, 5].map((lvl) => {
            const p = puzzles[`level-${lvl}`];
            if (!p) return null;
            return (
              <div key={lvl} class="print-puzzle">
                <h2>
                  {s.difficulty[lvl]} ({p.questions.length} questions)
                </h2>
                {p.questions.map((q, qi) => (
                  <div key={q.text} class="print-question">
                    <div class="print-question-text">
                      {qi + 1}. {q.text}
                    </div>
                    <div
                      class={`print-options ${q.options.some((o) => o.label.length > 12) ? "print-options-long" : ""}`}
                    >
                      {q.options.map((opt, oi) => (
                        <span key={opt.label} class="print-option">
                          {String.fromCharCode(65 + oi)}. {opt.label}
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

      {importPlan && (
        <ImportPreview
          plan={importPlan}
          onConfirm={handleConfirmImport}
          onCancel={() => setImportPlan(null)}
        />
      )}
    </>
  );
}

const ACTION_ORDER: ImportAction[] = [
  "new",
  "replace-completed",
  "replace-longer",
  "keep-completed",
  "keep-longer",
  "identical",
];

function ImportPreview({
  plan,
  onConfirm,
  onCancel,
}: {
  plan: ImportPlan;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const s = t();
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    ref.current?.showModal();
  }, []);

  const grouped = new Map<ImportAction, string[]>();
  for (const entry of plan.entries) {
    const list = grouped.get(entry.action) ?? [];
    list.push(entry.id);
    grouped.set(entry.action, list);
  }
  for (const list of grouped.values()) list.sort();
  const hasChanges = plan.entries.some(
    (e) => e.action === "new" || e.action === "replace-completed" || e.action === "replace-longer",
  );

  return (
    <dialog
      ref={ref}
      class="help-panel import-preview"
      onClose={onCancel}
      onClick={(e) => {
        if (e.target === ref.current) onCancel();
      }}
    >
      <div class="help-panel-header">
        <h3>{s.backup.importPreview}</h3>
        <button class="help-close" onClick={onCancel}>
          &times;
        </button>
      </div>
      <p class="import-summary">{s.backup.puzzlesInBackup(plan.entries.length)}</p>
      {ACTION_ORDER.map((action) => {
        const ids = grouped.get(action);
        if (!ids?.length) return null;
        return (
          <div key={action} class="import-section">
            <h4>
              {s.backup.actions[action]} ({ids.length})
            </h4>
            <ul class="import-list">
              {ids.map((id) => (
                <li key={id}>{id}</li>
              ))}
            </ul>
          </div>
        );
      })}
      <div class="import-actions">
        {hasChanges ? (
          <>
            <button class="onboarding-dismiss" onClick={onConfirm}>
              {s.backup.confirmImport}
            </button>
            <button class="help-close" onClick={onCancel} style={{ fontSize: "0.9rem" }}>
              {s.backup.cancel}
            </button>
          </>
        ) : (
          <button class="onboarding-dismiss" onClick={onCancel}>
            {s.backup.ok}
          </button>
        )}
      </div>
    </dialog>
  );
}

function PastPuzzlesPage() {
  const s = t();
  const [showHelp, setShowHelp] = useState(false);
  const today = todayDateStr();

  const dates: string[] = [];
  for (let i = 0; i < 30; i++) {
    const d = dateStrFromOffset(i);
    if (isValidDate(d)) dates.push(d);
  }

  return (
    <>
      <AppHeader
        onHelp={() => setShowHelp(true)}
        onExport={() => {
          const json = exportData();
          const blob = new Blob([json], { type: "application/json" });
          const url = URL.createObjectURL(blob);
          const a = document.createElement("a");
          a.href = url;
          a.download = "refpuzzle-backup.json";
          a.click();
          URL.revokeObjectURL(url);
        }}
        onImport={() => {}}
      />

      <div class="history-page">
        <h2>{s.daily.pastPuzzles}</h2>
        <div class="history-list">
          {dates.map((dateStr) => {
            const isCurrent = dateStr === today;
            const levels = [1, 2, 3, 4, 5].map((l) => {
              const { started, completed } = hasState(puzzleId(dateStr, l));
              return { level: l, started, completed };
            });
            const solved = levels.filter((l) => l.completed);
            const started = levels.filter((l) => l.started && !l.completed);
            let status: string;
            if (solved.length === 5) {
              status = s.daily.allSolved;
            } else if (solved.length > 0 || started.length > 0) {
              const parts: string[] = [];
              if (solved.length > 0)
                parts.push("✓ " + solved.map((l) => s.difficulty[l.level]).join(", "));
              if (started.length > 0)
                parts.push("• " + started.map((l) => s.difficulty[l.level]).join(", "));
              status = parts.join("  ");
            } else {
              status = s.daily.notStarted;
            }
            return (
              <a
                key={dateStr}
                href={`/day/${dateStr}`}
                class={`history-item ${isCurrent ? "history-today" : ""}`}
              >
                <span class="history-date">
                  {isCurrent ? s.daily.today : dateStr}
                  <span class="history-day"> {s.daily.dayNumber(dayNumber(dateStr))}</span>
                </span>
                <span class="history-progress">{status}</span>
              </a>
            );
          })}
        </div>
      </div>

      {showHelp && <HelpPanel onClose={() => setShowHelp(false)} />}
    </>
  );
}

function DayRoute() {
  const loc = useLocation();
  const dateStr = loc.path.replace("/day/", "");
  if (!dateStr || !isValidDate(dateStr)) {
    return (
      <div class="not-found">
        <h1>No puzzle</h1>
        <p>No puzzle available for this date.</p>
        <a href="/">Back to today</a>
      </div>
    );
  }
  return <DayView dateStr={dateStr} />;
}

function NotFound() {
  return (
    <div class="not-found">
      <h1>404</h1>
      <p>Page not found</p>
      <a href="/">Back to puzzles</a>
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
          <Route path="/day/:date" component={DayRoute} />
          <Route default component={NotFound} />
        </Router>
      </div>
    </LocationProvider>
  );
}
