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
  function cycle() {
    const html = document.documentElement;
    const current = html.getAttribute("data-theme");
    if (current === "dark") {
      html.setAttribute("data-theme", "light");
      localStorage.setItem("refpuzzle:theme", "light");
    } else if (current === "light") {
      html.removeAttribute("data-theme");
      localStorage.removeItem("refpuzzle:theme");
    } else {
      html.setAttribute("data-theme", "dark");
      localStorage.setItem("refpuzzle:theme", "dark");
    }
  }

  return (
    <button class="theme-toggle" onClick={cycle} aria-label="Toggle theme">
      theme
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
  const s = t();
  const [showHelp, setShowHelp] = useState(false);
  const [dateStr] = useState(todayDateStr);
  const [activeLevel, setActiveLevel] = useState(1);
  const [puzzles, setPuzzles] = useState<Record<string, Puzzle> | null>(null);
  const [loading, setLoading] = useState(true);
  const [puzzleVersion, setPuzzleVersion] = useState(0);

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
    if (activeLevel < 5) setActiveLevel(activeLevel + 1);
  }, [activeLevel]);

  return (
    <>
      <AppHeader onHelp={() => setShowHelp(true)} />
      <OnboardingBanner />

      <div class="daily-header">
        <span class="daily-date">Day #{dayNumber(dateStr)} &mdash; {dateStr}</span>
      </div>

      <div class="difficulty-tabs">
        {[1, 2, 3, 4, 5].map((level) => {
          const solved = loadState(puzzleId(dateStr, level))?.completed;
          return (
            <button
              key={level}
              class={`difficulty-tab ${activeLevel === level ? "active" : ""} ${solved ? "tab-solved" : ""}`}
              onClick={() => setActiveLevel(level)}
            >
              <span class="tab-label">{s.difficulty[level]}</span>
              {solved && <span class="tab-check"> &#10003;</span>}
            </button>
          );
        })}
      </div>

      {loading && <div class="loading">Loading...</div>}

      {!loading && !currentPuzzle && (
        <div class="loading">No puzzle available for today.</div>
      )}

      {!loading && currentPuzzle && (
        <PuzzleView
          key={`${pid}-${puzzleVersion}`}
          puzzle={currentPuzzle}
          initialHash={null}
          onNextPuzzle={handleNextLevel}
          onCompleted={() => setPuzzleVersion((v) => v + 1)}
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
            const solvedLevels = [1, 2, 3, 4, 5].filter(
              (l) => loadState(puzzleId(dateStr, l))?.completed,
            );
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
                <span class="history-progress">
                  {solvedLevels.length > 0
                    ? solvedLevels.map((l) => s.difficulty[l]).join(", ")
                    : "Not started"}
                </span>
              </a>
            );
          })}
        </div>
      </div>

      {showHelp && <HelpPanel onClose={() => setShowHelp(false)} />}
    </>
  );
}

function DayPage({ dateStr }: { dateStr: string }) {
  const s = t();
  const [showHelp, setShowHelp] = useState(false);
  const [activeLevel, setActiveLevel] = useState(1);
  const [puzzles, setPuzzles] = useState<Record<string, Puzzle> | null>(null);
  const [loading, setLoading] = useState(true);
  const [puzzleVersion, setPuzzleVersion] = useState(0);

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
    if (activeLevel < 5) setActiveLevel(activeLevel + 1);
  }, [activeLevel]);

  return (
    <>
      <AppHeader onHelp={() => setShowHelp(true)} />

      <div class="daily-header">
        <a href="/history" class="back-link">&larr; History</a>
        <span class="daily-date">Day #{dayNumber(dateStr)} &mdash; {dateStr}</span>
      </div>

      <div class="difficulty-tabs">
        {[1, 2, 3, 4, 5].map((level) => {
          const solved = loadState(puzzleId(dateStr, level))?.completed;
          return (
            <button
              key={level}
              class={`difficulty-tab ${activeLevel === level ? "active" : ""} ${solved ? "tab-solved" : ""}`}
              onClick={() => setActiveLevel(level)}
            >
              <span class="tab-label">{s.difficulty[level]}</span>
              {solved && <span class="tab-check"> &#10003;</span>}
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
          key={`${pid}-${puzzleVersion}`}
          puzzle={currentPuzzle}
          initialHash={null}
          onNextPuzzle={handleNextLevel}
          onCompleted={() => setPuzzleVersion((v) => v + 1)}
        />
      )}

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
  return <DayPage dateStr={dateStr} />;
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
