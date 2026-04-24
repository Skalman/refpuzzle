import { useState, useEffect, useCallback } from "preact/hooks";
import { LocationProvider, Router, Route, useLocation } from "preact-iso";
import { PuzzleView } from "./components/PuzzleView.tsx";
import { IconCalendar, IconHelp, IconMoon, IconSun, IconMonitor, IconPrint, IconCheck, IconX } from "./components/Icons.tsx";
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

function ThemeToggle() {
  const [mode, setMode] = useState(() => {
    return document.documentElement.getAttribute("data-theme") ?? "auto";
  });

  function cycle() {
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
  }

  return (
    <button class="theme-toggle" onClick={cycle} aria-label="Toggle theme">
      {mode === "dark" ? <IconMoon /> : mode === "light" ? <IconSun /> : <IconMonitor />}
    </button>
  );
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
          <li>{s.onboarding.step1} (<IconX size="0.9em" strokeWidth={3} class="icon-incorrect" />)</li>
          <li>{s.onboarding.step2} (<IconCheck size="0.9em" strokeWidth={3} class="icon-correct" />)</li>
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
  return (
    <div class="help-panel-overlay" onClick={onClose}>
      <div class="help-panel" onClick={(e) => e.stopPropagation()}>
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
              {i === 0 && <> (<IconX size="0.9em" strokeWidth={3} class="icon-incorrect" />)</>}
              {i === 1 && <> (<IconCheck size="0.9em" strokeWidth={3} class="icon-correct" />)</>}
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
          Inspired by <a href="https://www.logiquiz.com/" target="_blank" rel="noopener noreferrer">Logiquiz</a>
        </p>
      </div>
    </div>
  );
}

function AppHeader({ onHelp }: { onHelp: () => void }) {
  return (
    <header class="app-header">
      <h1>
        <a href="/" class="app-title-link">
          <Logo />
          <span class="app-title"><span class="app-title-ref">Ref</span>puzzle</span>
        </a>
      </h1>
      <div class="header-actions">
        <a href="/history" class="help-btn" aria-label="History" title="Past puzzles">
          <IconCalendar />
        </a>
        <button
          class="help-btn"
          onClick={onHelp}
          aria-label="Help"
          title="How to play"
        >
          <IconHelp />
        </button>
        <ThemeToggle />
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

  const params = new URLSearchParams(window.location.search);
  const hashLevel = Number(params.get("l")) || 0;
  const initialHash = window.location.hash.slice(1) || null;
  const [activeLevel, setActiveLevel] = useState(hashLevel >= 1 && hashLevel <= 5 ? hashLevel : 1);

  const selectLevel = useCallback((level: number) => {
    setActiveLevel(level);
    window.history.replaceState(null, "", `/day/${dateStr}?l=${level}`);
  }, [dateStr]);

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

  const isToday = dateStr === todayDateStr();

  return (
    <>
      <AppHeader onHelp={() => setShowHelp(true)} />
      {isToday && <OnboardingBanner />}

      <div class="daily-header">
        {!isToday && <a href="/history" class="back-link">&larr; History</a>}
        <span class="daily-date">Day #{dayNumber(dateStr)} &mdash; {dateStr}</span>
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
              {solved && <span class="tab-check">&#10003; </span>}
              {started && !solved && <span class="tab-started-dot">&#8226; </span>}
              <span class="tab-label">{s.difficulty[level]}</span>
            </button>
          );
        })}
      </div>

      {loading && <div class="loading">Loading...</div>}

      {!loading && !currentPuzzle && (
        <div class="loading">No puzzle available for this date.</div>
      )}

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
        <button class="print-btn" onClick={() => window.print()}><IconPrint size="0.9em" /> Print all puzzles</button>
      )}

      {puzzles && (
        <div class="print-only">
          <h1>Refpuzzle &mdash; Day #{dayNumber(dateStr)} &mdash; {dateStr}</h1>
          {[1, 2, 3, 4, 5].map((lvl) => {
            const p = puzzles[`level-${lvl}`];
            if (!p) return null;
            return (
              <div key={lvl} class="print-puzzle">
                <h2>{s.difficulty[lvl]} ({p.questions.length} questions)</h2>
                {p.questions.map((q, qi) => (
                  <div key={q.text} class="print-question">
                    <div class="print-question-text">{qi + 1}. {q.text}</div>
                    <div class={`print-options ${q.options.some((o) => o.label.length > 12) ? "print-options-long" : ""}`}>
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
    </>
  );
}

function HistoryPage() {
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
      <AppHeader onHelp={() => setShowHelp(true)} />

      <div class="history-page">
        <h2>Past Puzzles</h2>
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
              status = "All solved!";
            } else if (solved.length > 0 || started.length > 0) {
              const parts: string[] = [];
              if (solved.length > 0) parts.push("✓ " + solved.map((l) => s.difficulty[l.level]).join(", "));
              if (started.length > 0) parts.push("• " + started.map((l) => s.difficulty[l.level]).join(", "));
              status = parts.join("  ");
            } else {
              status = "Not started";
            }
            return (
              <a
                key={dateStr}
                href={`/day/${dateStr}`}
                class={`history-item ${isCurrent ? "history-today" : ""}`}
              >
                <span class="history-date">
                  {isCurrent ? "Today" : dateStr}
                  <span class="history-day"> Day #{dayNumber(dateStr)}</span>
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
          <Route path="/history" component={HistoryPage} />
          <Route path="/day/:date" component={DayRoute} />
          <Route default component={NotFound} />
        </Router>
      </div>
    </LocationProvider>
  );
}
