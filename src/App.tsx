import { useState, useEffect, useCallback, useRef } from "preact/hooks";
import { useForceUpdate, useRevalidated } from "./lib/hooks.ts";
import { LocationProvider, Router, Route, useLocation } from "preact-iso";
import { tinykeys } from "tinykeys";
import { PuzzleView } from "./components/PuzzleView.tsx";
import { KeyboardHelp } from "./components/KeyboardHelp.tsx";
import { planImport, applyImport } from "./lib/backup.ts";
import type { ImportPlan } from "./lib/backup.ts";
import { joinSync } from "./lib/sync.ts";
// QR components lazy-loaded via dynamic import (no preact dependency in chunks)
import type { Puzzle } from "./engine/types.ts";
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
import { decodePlaygroundHash } from "./lib/playground.ts";
import { guarded } from "./lib/keyboard.ts";
import { t } from "./i18n/index.ts";
import { replayLogoAnimation } from "./components/Logo.tsx";
import { ImportPreview } from "./components/ImportPreview.tsx";
import { AppHeader } from "./components/AppHeader.tsx";
import { ArchivePage } from "./components/ArchivePage.tsx";
import { useBackupFlow, BackupDialogs } from "./components/BackupFlow.tsx";
import { ErrorOverlay } from "./components/ErrorOverlay.tsx";
import { SafeAreaSimulator } from "./components/SafeAreaSimulator.tsx";
import { InlineHelp } from "./components/InlineHelp.tsx";
import { DifficultyTabs } from "./components/DifficultyTabs.tsx";
import { PrintSheet } from "./components/PrintSheet.tsx";
import { PageFooter } from "./components/PageFooter.tsx";
import { Loading } from "./components/ui/Loading.tsx";
import { NoticePage } from "./components/ui/NoticePage.tsx";
import { adoptDebugParam } from "./lib/debug.ts";

adoptDebugParam();

function DailyPage() {
  const dateStr = useToday();
  return <DayView dateStr={dateStr} />;
}

function DayView({ dateStr, initialLevel }: { dateStr: string; initialLevel?: number }) {
  const s = t();
  const { route } = useLocation();
  const [showKeyboardHelp, setShowKeyboardHelp] = useState(false);
  // Filled by the puzzle view; the header's Share row opens its sheet through it.
  const shareRef = useRef<{ open: () => void } | null>(null);
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

  const selectLevel = useCallback(
    (level: number) => {
      setActiveLevel(level);
      route(`/${dateStr}/${level}`, true);
      replayLogoAnimation();
    },
    [dateStr, route],
  );

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
        onShare={currentPuzzle ? () => shareRef.current?.open() : undefined}
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

      <DifficultyTabs dateStr={dateStr} activeLevel={activeLevel} onSelect={selectLevel} />

      {loading && <Loading />}

      {!loading && !currentPuzzle && <Loading>{s.app.noPuzzle}</Loading>}

      {!loading && currentPuzzle && (
        <PuzzleView
          key={pid}
          puzzle={currentPuzzle}
          dateStr={dateStr}
          level={activeLevel}
          initialHash={activeLevel === initialLevel ? initialHash : null}
          shareRef={shareRef}
          onNextPuzzle={handleNextLevel}
          onChanged={handleChanged}
        />
      )}

      {showKeyboardHelp && <KeyboardHelp onClose={() => setShowKeyboardHelp(false)} />}

      <InlineHelp />

      {puzzles && <PrintSheet dateStr={dateStr} puzzles={puzzles} />}

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
      <NoticePage>
        <h1>{s.notFound.noPuzzle}</h1>
        <p>{s.app.noPuzzle}</p>
        <a href="/">{s.notFound.backToToday}</a>
      </NoticePage>
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
    <NoticePage>
      {status === "joining" && <Loading />}
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
    </NoticePage>
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

  if (state.status === "loading") return <Loading />;
  if (state.status === "error") return <Loading>Invalid puzzle hash.</Loading>;
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
    <NoticePage>
      <h1>{s.notFound.title}</h1>
      <p>{s.notFound.pageNotFound}</p>
      <a href="/">{s.notFound.backToPuzzles}</a>
    </NoticePage>
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
        <PageFooter />
        {import.meta.env.DEV && <SafeAreaSimulator />}
      </div>
    </LocationProvider>
  );
}
