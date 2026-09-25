import { useState, useRef, useEffect, useCallback } from "preact/hooks";
import {
  IconCalendar,
  IconCheck,
  IconChevronDown,
  IconMoon,
  IconSun,
  IconSunMoon,
} from "./Icons.tsx";
import { Brand } from "./Brand.tsx";
import { Logo } from "./Logo.tsx";
import { ShareDialog } from "./ShareDialog.tsx";
import { SplitMenu } from "./SplitMenu.tsx";
import { MenuItem, MenuLink, MenuPopover } from "./ui/Menu.tsx";
import { Button, ButtonLink, buttonClass } from "./ui/Button.tsx";
import { t } from "../i18n/index.ts";
import { arrowNavHandler, menuNavHandler } from "../lib/keyboard.ts";
import { classNames, tw } from "../lib/classNames.ts";

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
 * `itemClass` adds to the rows, for how the host sets them apart.
 */
function ThemeOptions({
  theme,
  itemClass,
  onPick,
}: {
  theme: ReturnType<typeof useTheme>;
  itemClass?: string;
  onPick?: () => void;
}) {
  const s = t();
  return (
    <>
      {THEME_MODES.map((choice) => (
        <MenuItem
          key={choice}
          class={classNames(THEME_ROW, itemClass)}
          role="menuitemradio"
          aria-checked={theme.mode === choice}
          onClick={(e) => {
            e.stopPropagation();
            theme.select(choice);
            onPick?.();
          }}
        >
          <IconCheck
            size="0.9em"
            class="invisible flex-none text-accent group-aria-checked:visible"
          />
          {s.header.themeModes[choice]}
        </MenuItem>
      ))}
    </>
  );
}

/** A theme choice's row: the check shows on the chosen one. */
const THEME_ROW = tw`group aria-checked:font-semibold aria-checked:text-accent`;

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
  onShare,
  onBackup,
}: {
  onKeyboardHelp?: () => void;
  onPrint?: () => void;
  onShare?: () => void;
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
    for (const el of moreMenuRef.current?.querySelectorAll("[data-menu-item]") ?? []) {
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

  /** A menu row's click: the menu closes, then `action` runs. */
  const pick = (action: () => void) => () => {
    setMoreMenu(false);
    action();
  };

  return (
    <header class="relative mb-4 flex items-center justify-between">
      <h1 class="m-0 flex items-center gap-2 text-title font-normal">
        <Logo />
        <a href="/" class="inline-flex flex-col leading-[1.2]">
          <span class="tracking-[-0.02em]">
            <Brand />
            {import.meta.env.DEV && <span class="font-bold text-(--dev-badge)"> (dev)</span>}
          </span>
          <span class="hidden text-badge font-normal tracking-[0.03em] text-muted md:inline">
            {s.puzzleList.subtitle}
          </span>
        </a>
      </h1>
      <div
        class="flex items-center gap-2"
        role="toolbar"
        onKeyDown={arrowNavHandler("[data-toolbar-item]")}
      >
        <span class="hidden md:inline-flex">
          <ButtonLink
            variant="ghost"
            size="md-compact"
            href="/archive"
            tabIndex={0}
            icon={<IconCalendar />}
            data-toolbar-item
          >
            {s.daily.archive}
          </ButtonLink>
        </span>
        <span class="hidden items-stretch md:inline-flex">
          <Button
            variant="ghost"
            size="md-compact"
            class="rounded-r-none"
            data-toolbar-item
            tabIndex={-1}
            onClick={theme.toggle}
            aria-label={theme.toggleLabel}
            title={theme.toggleLabel}
            icon={theme.modeIcon}
          >
            {s.header.theme}
          </Button>
          <SplitMenu
            buttonClass={buttonClass({
              variant: "ghost",
              size: "md-compact",
              class: "self-stretch rounded-l-none border-l",
            })}
            tabIndex={-1}
            toolbarItem
            label={s.header.themeOptions}
          >
            {(close) => <ThemeOptions theme={theme} onPick={close} />}
          </SplitMenu>
        </span>
        <span class="relative">
          <Button
            ref={moreBtnRef}
            variant="outline-muted"
            class="font-bold tracking-widest"
            data-toolbar-item
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
          </Button>
          {moreMenu && (
            <MenuPopover ref={moreMenuRef} onKeyDown={handleMoreMenuKeyDown}>
              {/* Installed, the share dialog's App mode offers this link. */}
              {!(isInstalled && onShare) && (
                <MenuItem onClick={pick(() => setShowInstallInfo(true))}>
                  {isInstalled ? s.install.shareApp : s.install.button}
                </MenuItem>
              )}
              {onShare && <MenuItem onClick={pick(onShare)}>{s.share.share}</MenuItem>}
              <MenuLink mobileOnly href="/archive" onClick={() => setMoreMenu(false)}>
                {s.daily.archive}
              </MenuLink>
              <MenuItem
                ref={themeOptionsBtnRef}
                mobileOnly
                class="group"
                aria-expanded={themeOptions}
                onClick={(e) => {
                  e.stopPropagation();
                  setThemeOptions((v) => !v);
                }}
              >
                <IconChevronDown
                  size="0.9em"
                  class="transition-transform duration-150 group-aria-[expanded=false]:-rotate-90"
                />
                {s.header.theme}
              </MenuItem>
              {themeOptions && (
                // Under their disclosure, indented.
                <div class="md:hidden" role="group" aria-label={s.header.themeOptions}>
                  <ThemeOptions theme={theme} itemClass="pl-[1.6rem]" />
                </div>
              )}
              <hr class="m-0 border-t md:hidden" />
              {onKeyboardHelp && (
                <MenuItem desktopOnly onClick={pick(onKeyboardHelp)}>
                  {s.keyboard.title}
                </MenuItem>
              )}
              {onPrint && <MenuItem onClick={pick(onPrint)}>{s.daily.printAll}</MenuItem>}
              <MenuItem onClick={pick(onBackup)}>{s.backup.button}</MenuItem>
            </MenuPopover>
          )}
        </span>
      </div>
      {showInstallInfo && (
        <ShareDialog
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
