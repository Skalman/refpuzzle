import { useState, useEffect, useCallback } from "preact/hooks";
import { LocationProvider, Router, Route, useLocation } from "preact-iso";
import { PuzzleView } from "./components/PuzzleView.tsx";
import type { Puzzle } from "./engine/types.ts";
import {
  fetchDaily,
  todayDateStr,
  dayNumber,
  isValidDate,
  dateStrFromOffset,
  puzzleId,
} from "./puzzles/daily.ts";
import { loadState } from "./lib/store.ts";
import { t } from "./i18n/index.ts";
import { Logo } from "./components/Logo.tsx";

function ThemeToggle() {
  const [icon, setIcon] = useState(() => {
    const theme = document.documentElement.getAttribute("data-theme");
    return theme === "dark" ? "\u{1F319}" : theme === "light" ? "☀️" : "\u{1F310}";
  });

  function cycle() {
    const html = document.documentElement;
    const current = html.getAttribute("data-theme");
    if (current === "dark") {
      html.setAttribute("data-theme", "light");
      localStorage.setItem("refpuzzle:theme", "light");
      setIcon("☀️");
    } else if (current === "light") {
      html.removeAttribute("data-theme");
      localStorage.removeItem("refpuzzle:theme");
      setIcon("\u{1F310}");
    } else {
      html.setAttribute("data-theme", "dark");
      localStorage.setItem("refpuzzle:theme", "dark");
      setIcon("\u{1F319}");
    }
  }

  return (
    <button class="theme-toggle" onClick={cycle} aria-label="Toggle theme">
      {icon}
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
          <li>{s.onboarding.step1}</li>
          <li>{s.onboarding.step2}</li>
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
          {s.help.howToPlaySteps.map((step) => (
            <li key={step}>{step}</li>
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
      </div>
    </div>
  );
}

function AppHeader({ onHelp }: { onHelp: () => void }) {
  const s = t();
  return (
    <header class="app-header">
      <h1>
        <a href="/" style={{ textDecoration: "none", color: "inherit" }}>
          <Logo />
          {s.app.title}
        </a>
      </h1>
      <div class="header-actions">
        <a href="/history" class="help-btn" aria-label="History" title="Past puzzles">
          &#128197;
        </a>
        <button
          class="help-btn"
          onClick={onHelp}
          aria-label="Help"
          title="How to play"
        >
          ?
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
          const state = loadState(puzzleId(dateStr, level));
          const solved = !!state?.completed;
          const started = !!state && !solved;
          return (
            <button
              key={level}
              class={`difficulty-tab ${activeLevel === level ? "active" : ""} ${solved ? "tab-solved" : ""} ${started ? "tab-started" : ""}`}
              onClick={() => selectLevel(level)}
            >
              {solved && <span class="tab-check">&#10003; </span>}
              {started && <span class="tab-started-dot">&#8226; </span>}
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
          onChanged={() => setPuzzleVersion((v) => v + 1)}
        />
      )}

      {showHelp && <HelpPanel onClose={() => setShowHelp(false)} />}
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
              const state = loadState(puzzleId(dateStr, l));
              return { level: l, started: !!state, completed: !!state?.completed };
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
