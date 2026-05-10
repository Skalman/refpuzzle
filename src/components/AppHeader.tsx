import { useState, useRef, useEffect, useCallback } from "preact/hooks";
import { IconCalendar, IconMoon, IconSun, IconSunMoon } from "./Icons.tsx";
import { Logo } from "./Logo.tsx";
import { t } from "../i18n/index.ts";
import { arrowNavHandler } from "../lib/keyboard.ts";

function updateThemeColor(dark: boolean) {
  const meta = document.querySelector('meta[name="theme-color"]');
  if (meta) meta.setAttribute("content", dark ? "#0f1117" : "#f8f9fa");
}

export function useTheme() {
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
      updateThemeColor(false);
    } else if (current === "light") {
      html.removeAttribute("data-theme");
      localStorage.removeItem("refpuzzle:theme");
      setMode("auto");
      updateThemeColor(matchMedia("(prefers-color-scheme: dark)").matches);
    } else {
      html.setAttribute("data-theme", "dark");
      localStorage.setItem("refpuzzle:theme", "dark");
      setMode("dark");
      updateThemeColor(true);
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
                onClick={(e) => {
                  e.stopPropagation();
                  theme.cycle();
                }}
              >
                {theme.icon} {s.header.theme}
              </button>
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
        <>
          <div class="install-backdrop" onClick={() => setShowInstallInfo(false)} />
          <div
            class="install-toast"
            role="alertdialog"
            onKeyDown={(e) => {
              if (e.key === "Escape") setShowInstallInfo(false);
            }}
          >
            {install?.type === "native" && (
              <button
                class="primary-btn install-btn"
                ref={(el) => el?.focus()}
                onClick={() => install.fire()}
              >
                {s.install.button}
              </button>
            )}
            <p>{s.install.qrPrompt}</p>
            <div
              class="qr-image"
              ref={(el) => {
                if (!el) return;
                import("./QrCode.tsx").then(({ default: renderQrSvg }) => {
                  el.innerHTML = renderQrSvg(window.location.origin + "/");
                });
              }}
            />
            <p class="install-domain">{window.location.host}</p>
            {install?.type === "instructions" && <p>{install.message}</p>}
          </div>
        </>
      )}
    </header>
  );
}
