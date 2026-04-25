import { useRef, useEffect } from "preact/hooks";
import logoSvg from "../assets/logo.svg?raw";
import { t } from "../i18n/index.ts";

let replayFn: (() => void) | null = null;

export function replayLogoAnimation() {
  replayFn?.();
}

export function Logo() {
  const s = t();
  const ref = useRef<HTMLSpanElement>(null);
  useEffect(() => {
    const el = ref.current;
    if (!el) return undefined;
    const svg = el.querySelector("svg");
    if (!svg) return undefined;

    function replay() {
      svg!.classList.add("replay");
      void el!.offsetHeight;
      svg!.classList.remove("replay");
    }

    el.addEventListener("mouseenter", replay);
    el.addEventListener("click", replay);
    el.addEventListener("focus", replay);
    replayFn = replay;
    return () => {
      replayFn = null;
    };
  }, []);
  return (
    <span
      ref={ref}
      class="app-logo"
      tabIndex={0}
      role="img"
      aria-label={s.aria.logo}
      dangerouslySetInnerHTML={{ __html: logoSvg }}
    />
  );
}
