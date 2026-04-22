import { useRef, useEffect } from "preact/hooks";
import logoSvg from "../assets/logo.svg?raw";

export function Logo() {
  const ref = useRef<HTMLSpanElement>(null);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const dotsG = el.querySelector(".dots");
    if (!dotsG) return;
    let running = true;
    const lastDot = dotsG.querySelector("circle:last-child");
    if (lastDot) {
      lastDot.addEventListener("animationend", (e) => {
        if ("animationName" in e && e.animationName === "pulse") running = false;
      });
    }
    const g = dotsG;
    const wrapper = el;
    function restartPulse() {
      if (running) return;
      running = true;
      g.classList.add("no-anim", "once");
      void wrapper.offsetHeight;
      g.classList.remove("no-anim");
    }
    wrapper.addEventListener("mouseenter", restartPulse);
    wrapper.addEventListener("click", restartPulse);
  }, []);
  return <span ref={ref} class="app-logo" dangerouslySetInnerHTML={{ __html: logoSvg }} />;
}
