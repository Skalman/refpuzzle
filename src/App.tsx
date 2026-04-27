import { useState, useEffect, useCallback, useRef } from "preact/hooks";
import { LocationProvider, Router, Route, useLocation } from "preact-iso";
import { tinykeys } from "tinykeys";
import { PuzzleView } from "./components/PuzzleView.tsx";
import { KeyboardHelp, KeyboardShortcutList } from "./components/KeyboardHelp.tsx";
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
import { startSync, joinSync, pollSync } from "./lib/sync.ts";
import type { Puzzle } from "./engine/types.ts";
import { renderQuestionText, renderOptionLabel, renderClaimLabel } from "./engine/render.ts";
import {
  fetchDaily,
  todayDateStr,
  dayNumber,
  isValidDate,
  dateStrFromOffset,
  puzzleId,
} from "./puzzles/daily.ts";
import { hasState } from "./lib/store.ts";
import { guarded, arrowNavHandler } from "./lib/keyboard.ts";
import { t } from "./i18n/index.ts";
import { Logo, replayLogoAnimation } from "./components/Logo.tsx";

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

type InstallState =
  | { type: "native"; fire: () => void }
  | { type: "instructions"; message: string }
  | { type: "qr" }
  | null;

function useInstall(): InstallState {
  const [state, setState] = useState<InstallState>(null);
  const s = t();

  useEffect(() => {
    if (window.matchMedia("(display-mode: standalone)").matches) return undefined;

    function onPrompt(e: Event) {
      e.preventDefault();
      const ev = e;
      setState({
        type: "native",
        fire: () => {
          if ("prompt" in ev && typeof ev.prompt === "function") ev.prompt();
        },
      });
    }
    window.addEventListener("beforeinstallprompt", onPrompt);

    const ua = navigator.userAgent;
    const isIOS = /iPad|iPhone|iPod/.test(ua);
    const isAndroidFF = /Android/.test(ua) && /Firefox/.test(ua);

    if (isIOS) {
      setState({ type: "instructions", message: s.install.iosSafari });
    } else if (isAndroidFF) {
      setState({ type: "instructions", message: s.install.androidFirefox });
    } else {
      // Desktop: wait briefly for beforeinstallprompt, fall back to QR code
      const timer = setTimeout(() => {
        setState((cur) => cur ?? { type: "qr" });
      }, 1000);
      return () => {
        clearTimeout(timer);
        window.removeEventListener("beforeinstallprompt", onPrompt);
      };
    }

    return () => window.removeEventListener("beforeinstallprompt", onPrompt);
  }, [s]);

  return state;
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
            {s.onboarding.step1}{" "}
            <span class="nowrap">(<IconX size="0.9em" strokeWidth={3} class="icon-incorrect" />)</span>
          </li>
          <li>
            {s.onboarding.step2}{" "}
            <span class="nowrap">(<IconCheck size="0.9em" strokeWidth={3} class="icon-correct" />)</span>
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
      <div class="help-panel-inner">
        <div class="help-panel-header">
          <h3>{s.help.title}</h3>
          <button class="help-close" onClick={onClose} aria-label={s.aria.close}>
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
                  <span class="nowrap">(<IconX size="0.9em" strokeWidth={3} class="icon-incorrect" />)</span>
                </>
              )}
              {i === 1 && (
                <>
                  {" "}
                  <span class="nowrap">(<IconCheck size="0.9em" strokeWidth={3} class="icon-correct" />)</span>
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
        <h4>{s.keyboard.title}</h4>
        <KeyboardShortcutList />
        <p class="help-credit">
          {s.help.inspiredBy}{" "}
          <a href="https://www.logiquiz.com/" target="_blank" rel="noopener noreferrer">
            Logiquiz
          </a>
        </p>
      </div>
    </dialog>
  );
}

function AppHeader({
  onHelp,
  onPrint,
  onSync,
  onExport,
  onImport,
}: {
  onHelp: () => void;
  onPrint?: () => void;
  onSync: () => void;
  onExport: () => void;
  onImport: (e: Event) => void;
}) {
  const s = t();
  const theme = useTheme();
  const install = useInstall();
  const [showInstallInfo, setShowInstallInfo] = useState(false);
  const [moreMenu, setMoreMenu] = useState(false);
  const moreBtnRef = useRef<HTMLButtonElement>(null);
  const moreMenuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!moreMenu) return undefined;
    const close = () => setMoreMenu(false);
    document.addEventListener("click", close);
    // Auto-focus the first visible item
    requestAnimationFrame(() => {
      const items = moreMenuRef.current?.querySelectorAll(".more-menu-item");
      if (items) {
        for (const item of items) {
          if (item instanceof HTMLElement && item.offsetParent !== null) {
            item.focus();
            break;
          }
        }
      }
    });
    return () => document.removeEventListener("click", close);
  }, [moreMenu]);

  function handleMoreMenuKeyDown(e: KeyboardEvent) {
    const allItems = moreMenuRef.current?.querySelectorAll(".more-menu-item");
    if (!allItems) return;
    const items: HTMLElement[] = [];
    for (const el of allItems) {
      if (el instanceof HTMLElement && el.offsetParent !== null) items.push(el);
    }
    if (!items.length) return;

    const current = document.activeElement;
    const idx = current instanceof HTMLElement ? items.indexOf(current) : -1;

    if (e.key === "ArrowDown") {
      e.preventDefault();
      items[(idx + 1) % items.length].focus();
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      items[(idx - 1 + items.length) % items.length].focus();
    } else if (e.key === "Escape") {
      e.preventDefault();
      setMoreMenu(false);
      moreBtnRef.current?.focus();
    }
  }

  return (
    <header class="app-header">
      <h1>
        <Logo />
        <a href="/" class="app-title-link">
          <span class="app-title">
            <span class="app-title-ref">Ref</span>puzzle
          </span>
          <span class="app-tagline hide-mobile">{s.puzzleList.subtitle}</span>
        </a>
      </h1>
      <div class="header-actions" role="toolbar" onKeyDown={arrowNavHandler(".header-btn")}>
        <a
          href="/past"
          class="header-btn hide-mobile"
          tabIndex={0}
          aria-label={s.daily.pastPuzzles}
        >
          <IconCalendar /> {s.daily.pastPuzzles}
        </a>
        <button
          class="header-btn hide-mobile"
          tabIndex={-1}
          onClick={onHelp}
          aria-label={s.aria.help}
        >
          <IconHelp /> {s.help.title}
        </button>
        <button
          class="header-btn hide-mobile"
          tabIndex={-1}
          onClick={theme.cycle}
          aria-label={s.aria.toggleTheme}
        >
          {theme.icon} {s.header.theme}
        </button>
        <span class="more-menu-wrapper">
          <button
            ref={moreBtnRef}
            class="header-btn more-btn"
            tabIndex={-1}
            onClick={(e) => {
              e.stopPropagation();
              setMoreMenu((v) => !v);
            }}
            aria-label={s.aria.more}
            aria-haspopup="true"
            aria-expanded={moreMenu}
          >
            ⋯
          </button>
          {moreMenu && (
            <div ref={moreMenuRef} class="more-menu" role="menu" onKeyDown={handleMoreMenuKeyDown}>
              {install && (
                <button
                  class="more-menu-item"
                  role="menuitem"
                  onClick={() => {
                    setMoreMenu(false);
                    setShowInstallInfo(true);
                  }}
                >
                  {s.install.button}
                </button>
              )}
              <a
                href="/past"
                class="more-menu-item show-mobile"
                role="menuitem"
                onClick={() => setMoreMenu(false)}
              >
                {s.daily.pastPuzzles}
              </a>
              <button
                class="more-menu-item show-mobile"
                role="menuitem"
                onClick={() => {
                  setMoreMenu(false);
                  onHelp();
                }}
              >
                {s.help.title}
              </button>
              <button
                class="more-menu-item show-mobile"
                role="menuitem"
                onClick={(e) => {
                  e.stopPropagation();
                  theme.cycle();
                }}
              >
                {theme.icon} {s.header.theme}
              </button>
              <hr class="more-menu-divider show-mobile" />
              {onPrint && (
                <button
                  class="more-menu-item"
                  role="menuitem"
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
                role="menuitem"
                onClick={() => {
                  setMoreMenu(false);
                  onSync();
                }}
              >
                {s.sync.title}
              </button>
              <button
                class="more-menu-item"
                role="menuitem"
                onClick={() => {
                  setMoreMenu(false);
                  onExport();
                }}
              >
                {s.backup.exportBackup}
              </button>
              <label
                class="more-menu-item"
                role="menuitem"
                tabIndex={0}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    const input = e.currentTarget.querySelector("input");
                    if (input instanceof HTMLInputElement) input.click();
                  }
                }}
              >
                {s.backup.importBackup}
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
      {showInstallInfo && install && (
        <div
          class="install-toast"
          role="alertdialog"
          onClick={() => setShowInstallInfo(false)}
          onKeyDown={(e) => {
            if (e.key === "Escape") setShowInstallInfo(false);
          }}
        >
          <p>{s.install.qrPrompt}</p>
          <img src="/qr-install.png" alt="QR code" class="qr-image" width="120" height="120" />
          {install.type === "instructions" && <p>{install.message}</p>}
          <div class="install-toast-actions">
            {install.type === "native" && (
              <button class="onboarding-dismiss" onClick={() => install.fire()}>
                {s.install.button}
              </button>
            )}
            <button
              class="onboarding-dismiss"
              ref={(el) => el?.focus()}
              onClick={() => setShowInstallInfo(false)}
            >
              {s.backup.ok}
            </button>
          </div>
        </div>
      )}
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
  const [showKeyboardHelp, setShowKeyboardHelp] = useState(false);
  const [puzzles, setPuzzles] = useState<Record<string, Puzzle> | null>(null);
  const [loading, setLoading] = useState(true);
  const [_puzzleVersion, setPuzzleVersion] = useState(0);
  const [importPlan, setImportPlan] = useState<ImportPlan | null>(null);
  const [showSync, setShowSync] = useState(false);

  const params = new URLSearchParams(window.location.search);
  const hashLevel = Number(params.get("l")) || 0;
  const initialHash = window.location.hash.slice(1) || null;
  const [activeLevel, setActiveLevel] = useState(hashLevel >= 1 && hashLevel <= 5 ? hashLevel : 1);

  const activeLevelRef = useRef(activeLevel);
  activeLevelRef.current = activeLevel;
  const showKeyboardHelpRef = useRef(false);

  const selectLevel = useCallback(
    (level: number) => {
      setActiveLevel(level);
      window.history.replaceState(null, "", `/day/${dateStr}?l=${level}`);
      replayLogoAnimation();
    },
    [dateStr],
  );

  // Page-level keyboard shortcuts
  useEffect(() => {
    const g = guarded;
    const unsubscribe = tinykeys(window, {
      "[": g(() => {
        if (activeLevelRef.current > 1) selectLevel(activeLevelRef.current - 1);
      }),
      "]": g(() => {
        if (activeLevelRef.current < 5) selectLevel(activeLevelRef.current + 1);
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
        onSync={() => setShowSync(true)}
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

      <div class="difficulty-tabs" role="tablist" onKeyDown={arrowNavHandler(".difficulty-tab")}>
        {[1, 2, 3, 4, 5].map((level) => {
          const state = hasState(puzzleId(dateStr, level));
          const { started, completed: solved } = state;
          return (
            <button
              key={level}
              role="tab"
              aria-selected={activeLevel === level}
              tabIndex={activeLevel === level ? 0 : -1}
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
      {showKeyboardHelp && (
        <KeyboardHelp
          onClose={() => {
            setShowKeyboardHelp(false);
            showKeyboardHelpRef.current = false;
          }}
        />
      )}

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
            {s.app.title} &mdash; {s.daily.dayLabel(dayNumber(dateStr), dateStr)}
          </h1>
          {[1, 2, 3, 4, 5].map((lvl) => {
            const p = puzzles[`level-${lvl}`];
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
                      : renderOptionLabel(q.rule, opt.value, oi),
                  );
                  return (
                    // oxlint-disable-next-line react/no-array-index-key
                    <div key={qi} class="print-question">
                      <div class="print-question-text">
                        {qi + 1}. {renderQuestionText(q.rule)}
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

      {showSync && (
        <SyncDialog
          onImport={(json) => {
            setShowSync(false);
            try {
              setImportPlan(planImport(json));
            } catch (err) {
              alert(s.backup.importFailed(err instanceof Error ? err.message : "unknown error"));
            }
          }}
          onClose={() => setShowSync(false)}
        />
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
      <div class="help-panel-inner">
        <div class="help-panel-header">
          <h3>{s.backup.importPreview}</h3>
          <button class="help-close" onClick={onCancel} aria-label={s.aria.close}>
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
      </div>
    </dialog>
  );
}

function SyncDialog({
  onImport,
  onClose,
}: {
  onImport: (json: string) => void;
  onClose: () => void;
}) {
  const s = t();
  const ref = useRef<HTMLDialogElement>(null);
  const [code, setCode] = useState<string | null>(null);
  const [inputCode, setInputCode] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    ref.current?.showModal();
    return () => {
      if (pollRef.current) clearInterval(pollRef.current);
    };
  }, []);

  function handleStart() {
    setBusy(true);
    setError(null);
    startSync()
      .then((c) => {
        setCode(c);
        setBusy(false);
        pollRef.current = setInterval(() => {
          pollSync(c).then((json) => {
            if (json) {
              if (pollRef.current) clearInterval(pollRef.current);
              pollRef.current = null;
              onImport(json);
            }
          });
        }, 2000);
      })
      .catch(() => {
        setError(s.sync.error);
        setBusy(false);
      });
  }

  function handleJoin() {
    const trimmed = inputCode.trim();
    if (trimmed.length !== 6) return;
    setBusy(true);
    setError(null);
    joinSync(trimmed)
      .then((json) => {
        onImport(json);
      })
      .catch(() => {
        setError(s.sync.expired);
        setBusy(false);
      });
  }

  return (
    <dialog
      ref={ref}
      class="help-panel sync-dialog"
      onClose={onClose}
      onClick={(e) => {
        if (e.target === ref.current) onClose();
      }}
    >
      <div class="help-panel-inner">
        <div class="help-panel-header">
          <h3>{s.sync.title}</h3>
          <button class="help-close" onClick={onClose} aria-label={s.aria.close}>
            &times;
          </button>
        </div>
        <p>{s.sync.description}</p>

        {!code && (
          <>
            <button class="onboarding-dismiss sync-start-btn" onClick={handleStart} disabled={busy}>
              {s.sync.start}
            </button>

            <div class="sync-divider">
              <span>{s.sync.enterCode}</span>
            </div>

            <div class="sync-join">
              <input
                type="text"
                inputMode="numeric"
                pattern="[0-9]*"
                class="sync-code-input"
                maxLength={6}
                placeholder={s.sync.codePlaceholder}
                value={inputCode}
                onInput={(e) => {
                  const el = e.target;
                  if (el instanceof HTMLInputElement) setInputCode(el.value);
                }}
                onKeyDown={(e) => {
                  if (e.key === "Enter") handleJoin();
                }}
              />
              <button
                class="onboarding-dismiss"
                onClick={handleJoin}
                disabled={busy || inputCode.trim().length !== 6}
              >
                {s.sync.join}
              </button>
            </div>
          </>
        )}

        {code && (
          <div class="sync-waiting">
            <div class="sync-code-display">{code}</div>
            <p class="sync-waiting-text">{s.sync.waiting}</p>
          </div>
        )}

        {error && <p class="sync-error">{error}</p>}
      </div>
    </dialog>
  );
}

function DayItem({ dateStr, isToday }: { dateStr: string; isToday: boolean }) {
  const s = t();
  const levels = [1, 2, 3, 4, 5].map((l) => {
    const { started, completed } = hasState(puzzleId(dateStr, l));
    return { level: l, started, completed };
  });
  const solved = levels.filter((l) => l.completed);
  const started = levels.filter((l) => l.started && !l.completed);
  return (
    <a href={`/day/${dateStr}`} class={`history-item ${isToday ? "history-today" : ""}`}>
      <span class="history-date">
        {isToday ? s.daily.today : dateStr}
        <span class="history-day"> {s.daily.dayNumber(dayNumber(dateStr))}</span>
      </span>
      <span class="history-progress">
        {solved.length === 5 ? (
          s.daily.allSolved
        ) : solved.length > 0 || started.length > 0 ? (
          <>
            {solved.length > 0 && (
              <span>
                <IconCheck size="0.9em" strokeWidth={3} class="icon-correct" />{" "}
                {solved.map((l) => s.difficulty[l.level]).join(", ")}
              </span>
            )}
            {solved.length > 0 && started.length > 0 && "  "}
            {started.length > 0 && (
              <span>&#8226; {started.map((l) => s.difficulty[l.level]).join(", ")}</span>
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
  const [showHelp, setShowHelp] = useState(false);
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
      <AppHeader
        onHelp={() => setShowHelp(true)}
        onSync={() => {}}
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

      {showHelp && <HelpPanel onClose={() => setShowHelp(false)} />}
    </>
  );
}

function DayRoute() {
  const s = t();
  const loc = useLocation();
  const dateStr = loc.path.replace("/day/", "");
  if (!dateStr || !isValidDate(dateStr)) {
    return (
      <div class="not-found">
        <h1>{s.notFound.noPuzzle}</h1>
        <p>{s.app.noPuzzle}</p>
        <a href="/">{s.notFound.backToToday}</a>
      </div>
    );
  }
  return <DayView dateStr={dateStr} />;
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
          <Route path="/day/:date" component={DayRoute} />
          <Route default component={NotFound} />
        </Router>
      </div>
    </LocationProvider>
  );
}
