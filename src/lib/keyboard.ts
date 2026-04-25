export function arrowNavHandler(selector: string) {
  return (e: KeyboardEvent) => {
    if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
    const container = e.currentTarget;
    if (!(container instanceof HTMLElement)) return;
    const items: HTMLElement[] = [];
    for (const el of container.querySelectorAll(selector)) {
      if (el instanceof HTMLElement && el.offsetParent !== null) items.push(el);
    }
    if (items.length < 2) return;
    const current = document.activeElement;
    const idx = current instanceof HTMLElement ? items.indexOf(current) : -1;
    if (idx < 0) return;
    e.preventDefault();
    const next =
      e.key === "ArrowRight" ? (idx + 1) % items.length : (idx - 1 + items.length) % items.length;
    items[idx].setAttribute("tabindex", "-1");
    items[next].setAttribute("tabindex", "0");
    items[next].focus();
  };
}

export function initRovingTabindex(
  container: HTMLElement | null,
  selector: string,
  activeIndex?: number,
) {
  if (!container) return;
  const items = container.querySelectorAll(selector);
  let set = false;
  items.forEach((el, i) => {
    if (activeIndex != null && i === activeIndex) {
      el.setAttribute("tabindex", "0");
      set = true;
    } else {
      el.setAttribute("tabindex", "-1");
    }
  });
  if (!set && items.length > 0) {
    items[0].setAttribute("tabindex", "0");
  }
}

export function guarded(fn: (e: KeyboardEvent) => void) {
  return (e: KeyboardEvent) => {
    const t = e.target;
    if (
      t instanceof HTMLElement &&
      (t.closest("dialog") ||
        t.tagName === "INPUT" ||
        t.tagName === "TEXTAREA" ||
        t.tagName === "SELECT")
    )
      return;
    fn(e);
  };
}
