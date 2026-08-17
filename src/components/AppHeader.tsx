import { useState, useRef, useEffect, useCallback } from "preact/hooks";
import {
  IconCalendar,
  IconCheck,
  IconChevronDown,
  IconMoon,
  IconSun,
  IconSunMoon,
} from "./Icons.tsx";
import { Logo } from "./Logo.tsx";
import { ShareSheet } from "./ShareSheet.tsx";
import { SplitMenu } from "./SplitMenu.tsx";
import { t } from "../i18n/index.ts";
import { arrowNavHandler, menuNavHandler } from "../lib/keyboard.ts";

if (import.meta.env.DEV) document.title = `(dev) ${document.title}`;

const THEME_MODES = ["auto", "light", "dark"] as const;
type ThemeMode = (typeof THEME_MODES)[number];
type Appearance = "light" | "dark";

const DARK_QUERY = "(prefers-color-scheme: dark)";
const THEME_KEY = "refpuzzle:theme";

// Reads the resolved --bg rather than repeating the palette here, so the
// address-bar tint can't drift from the stylesheet. Runs after data-theme is
// on the root, so the computed value is already the new theme's.
function updateThemeColor() {
  const meta = document.querySelector('meta[name="theme-color"]');
  if (!meta) return;
  const bg = getComputedStyle(document.documentElement).getPropertyValue("--bg").trim();
  if (bg) meta.setAttribute("content", bg);
}

function themeIcon(mode: ThemeMode) {
  return mode === "light" ? <IconSun /> : mode === "dark" ? <IconMoon /> : <IconSunMoon />;
}

export function useTheme() {
  const s = t();
  const [mode, setMode] = useState<ThemeMode>(() => {
    const attr = document.documentElement.getAttribute("data-theme");
    return attr === "light" || attr === "dark" ? attr : "auto";
  });
  const [systemDark, setSystemDark] = useState(() => matchMedia(DARK_QUERY).matches);

  // The toggle's target and label depend on the system preference in every mode,
  // not just auto, so this tracks it unconditionally.
  useEffect(() => {
    const query = matchMedia(DARK_QUERY);
    const onChange = (e: MediaQueryListEvent) => setSystemDark(e.matches);
    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  }, []);

  const system: Appearance = systemDark ? "dark" : "light";
  const resolved: Appearance = mode === "auto" ? system : mode;

  useEffect(() => {
    updateThemeColor();
  }, [resolved]);

  // Every press flips the appearance. It lands on auto whenever auto already
  // resolves to the appearance being switched to, so a press never looks dead.
  const flipped: Appearance = resolved === "dark" ? "light" : "dark";
  const target: ThemeMode = system === flipped ? "auto" : flipped;

  const select = useCallback((next: ThemeMode) => {
    const html = document.documentElement;
    if (next === "auto") {
      html.removeAttribute("data-theme");
      localStorage.removeItem(THEME_KEY);
    } else {
      html.setAttribute("data-theme", next);
      localStorage.setItem(THEME_KEY, next);
    }
    setMode(next);
  }, []);

  const toggle = useCallback(() => select(target), [select, target]);

  // The icon reports the mode you are in; the label names where a press lands.
  return {
    mode,
    select,
    toggle,
    modeIcon: themeIcon(mode),
    toggleLabel: s.header.themeToggle[target],
  };
}

/**
 * The explicit Auto / Light / Dark choice, revealed by a disclosure: the
 * header's split-button popup and the ⋯ menu's expanded block both render it.
 * `itemClass` enrolls the rows in whichever host's arrow-key walk surrounds them.
 */
function ThemeOptions({
  theme,
  itemClass,
  onPick,
}: {
  theme: ReturnType<typeof useTheme>;
  itemClass: string;
  onPick?: () => void;
}) {
  const s = t();
  return (
    <>
      {THEME_MODES.map((choice) => (
        <button
          key={choice}
          class={itemClass}
          role="menuitemradio"
          aria-checked={theme.mode === choice}
          onClick={(e) => {
            e.stopPropagation();
            theme.select(choice);
            onPick?.();
          }}
        >
          <IconCheck size="0.9em" class="theme-option-check" />
          {s.header.themeModes[choice]}
        </button>
      ))}
    </>
  );
}

type InstallState =
  | { type: "native"; fire: () => void }
  | { type: "instructions"; message: string }
  | { type: "qr" }
  | null;

export function useInstall(): InstallState {
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

export function AppHeader({
  onKeyboardHelp,
  onPrint,
  onBackup,
}: {
  onKeyboardHelp?: () => void;
  onPrint?: () => void;
  onBackup: () => void;
}) {
  const s = t();
  const theme = useTheme();
  const install = useInstall();
  const isInstalled = window.matchMedia("(display-mode: standalone)").matches;
  const [showInstallInfo, setShowInstallInfo] = useState(false);
  const [moreMenu, setMoreMenu] = useState(false);
  const [themeOptions, setThemeOptions] = useState(false);
  const moreBtnRef = useRef<HTMLButtonElement>(null);
  const moreMenuRef = useRef<HTMLDivElement>(null);
  const themeOptionsBtnRef = useRef<HTMLButtonElement>(null);

  // Rows the viewport breakpoint hides have no offsetParent; skip those.
  function visibleMenuItems(): HTMLElement[] {
    const items: HTMLElement[] = [];
    for (const el of moreMenuRef.current?.querySelectorAll(".more-menu-item") ?? []) {
      if (el instanceof HTMLElement && el.offsetParent !== null) items.push(el);
    }
    return items;
  }

  // The disclosure starts collapsed every time the menu opens.
  useEffect(() => {
    if (!moreMenu) setThemeOptions(false);
  }, [moreMenu]);

  useEffect(() => {
    if (!moreMenu) return undefined;
    const close = () => setMoreMenu(false);
    document.addEventListener("click", close);
    requestAnimationFrame(() => visibleMenuItems()[0]?.focus());
    return () => document.removeEventListener("click", close);
  }, [moreMenu]);

  const handleMoreMenuKeyDown = menuNavHandler(visibleMenuItems, () => {
    // Escape collapses the theme disclosure first, then closes the menu.
    if (themeOptions) {
      setThemeOptions(false);
      themeOptionsBtnRef.current?.focus();
      return;
    }
    setMoreMenu(false);
    moreBtnRef.current?.focus();
  });

  return (
    <header class="app-header">
      <h1>
        <Logo />
        <a href="/" class="app-title-link">
          <span class="app-title">
            <span class="app-title-ref">Ref</span>puzzle
            {import.meta.env.DEV && <span class="dev-badge"> (dev)</span>}
          </span>
          <span class="app-tagline hide-mobile">{s.puzzleList.subtitle}</span>
        </a>
      </h1>
      <div class="header-actions" role="toolbar" onKeyDown={arrowNavHandler(".header-btn")}>
        <a href="/archive" class="header-btn hide-mobile" tabIndex={0}>
          <IconCalendar /> {s.daily.archive}
        </a>
        <span class="split-btn hide-mobile">
          <button
            class="header-btn"
            tabIndex={-1}
            onClick={theme.toggle}
            aria-label={theme.toggleLabel}
            title={theme.toggleLabel}
          >
            {theme.modeIcon} {s.header.theme}
          </button>
          <SplitMenu buttonClass="header-btn" tabIndex={-1} label={s.header.themeOptions}>
            {(close) => <ThemeOptions theme={theme} itemClass="theme-option" onPick={close} />}
          </SplitMenu>
        </span>
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
              <button
                class="more-menu-item"
                role="menuitem"
                onClick={() => {
                  setMoreMenu(false);
                  setShowInstallInfo(true);
                }}
              >
                {isInstalled ? s.install.shareApp : s.install.button}
              </button>
              <a
                href="/archive"
                class="more-menu-item show-mobile"
                role="menuitem"
                onClick={() => setMoreMenu(false)}
              >
                {s.daily.archive}
              </a>
              <button
                ref={themeOptionsBtnRef}
                class="more-menu-item show-mobile"
                role="menuitem"
                aria-expanded={themeOptions}
                onClick={(e) => {
                  e.stopPropagation();
                  setThemeOptions((v) => !v);
                }}
              >
                <IconChevronDown size="0.9em" class="disclosure-chevron" /> {s.header.theme}
              </button>
              {themeOptions && (
                <div class="show-mobile" role="group" aria-label={s.header.themeOptions}>
                  <ThemeOptions theme={theme} itemClass="more-menu-item theme-option" />
                </div>
              )}
              <hr class="more-menu-divider show-mobile" />
              {onKeyboardHelp && (
                <button
                  class="more-menu-item hide-mobile"
                  role="menuitem"
                  onClick={() => {
                    setMoreMenu(false);
                    onKeyboardHelp();
                  }}
                >
                  {s.keyboard.title}
                </button>
              )}
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
                  onBackup();
                }}
              >
                {s.backup.button}
              </button>
            </div>
          )}
        </span>
      </div>
      {showInstallInfo && (
        <ShareSheet
          url={`${window.location.origin}/`}
          title={isInstalled ? s.install.shareApp : s.install.button}
          onClose={() => setShowInstallInfo(false)}
          installAction={install?.type === "native" ? install.fire : undefined}
          installMessage={install?.type === "instructions" ? install.message : undefined}
        />
      )}
    </header>
  );
}
