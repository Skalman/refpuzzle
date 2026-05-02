// Inlined Lucide icons (https://lucide.dev, MIT license)
// Each is a minimal SVG at 24x24 viewBox, rendered at 1em.

interface Props {
  size?: string;
  class?: string;
  strokeWidth?: number;
}

const defaults = { size: "1em" };

function I({ d, size, class: cls, strokeWidth }: Props & { d: string }) {
  const s = size ?? defaults.size;
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      width={s}
      height={s}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width={strokeWidth ?? 2}
      stroke-linecap="round"
      stroke-linejoin="round"
      class={cls}
    >
      <path d={d} />
    </svg>
  );
}

function IM({ paths, size, class: cls, strokeWidth }: Props & { paths: string[] }) {
  const s = size ?? defaults.size;
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      width={s}
      height={s}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width={strokeWidth ?? 2}
      stroke-linecap="round"
      stroke-linejoin="round"
      class={cls}
    >
      {paths.map((d) => (
        <path key={d} d={d} />
      ))}
    </svg>
  );
}

// Undo
export function IconUndo(p: Props) {
  return <IM {...p} paths={["M3 7v6h6", "M21 17a9 9 0 0 0-9-9 9 9 0 0 0-6 2.3L3 13"]} />;
}

// Redo
export function IconRedo(p: Props) {
  return <IM {...p} paths={["M21 7v6h-6", "M3 17a9 9 0 0 1 9-9 9 9 0 0 1 6 2.3l3 2.7"]} />;
}

// Bookmark/Pin (checkpoint)
export function IconPin(p: Props) {
  return (
    <IM
      {...p}
      paths={[
        "M12 17v5",
        "M9 10.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V7a1 1 0 0 1 1-1 2 2 0 0 0 0-4H8a2 2 0 0 0 0 4 1 1 0 0 1 1 1z",
      ]}
    />
  );
}

// Lightbulb (hint)
export function IconHint(p: Props) {
  return (
    <IM
      {...p}
      paths={[
        "M15 14c.2-1 .7-1.7 1.5-2.5 1-.9 1.5-2.2 1.5-3.5A6 6 0 0 0 6 8c0 1 .2 2.2 1.5 3.5.7.7 1.3 1.5 1.5 2.5",
        "M9 18h6",
        "M10 22h4",
      ]}
    />
  );
}

// Check (correct mark)
export function IconCheck(p: Props) {
  return <I {...p} d="M20 6 9 17l-5-5" />;
}

// X (incorrect mark)
export function IconX(p: Props) {
  return <IM {...p} paths={["M18 6 6 18", "m6 6 12 12"]} />;
}

// Play (start)
export function IconPlay(p: Props) {
  return <I {...p} d="M6 3l14 9-14 9V3z" />;
}

// Chevron down (dropdown)
export function IconChevronDown(p: Props) {
  return <I {...p} d="m6 9 6 6 6-6" />;
}

// Refresh (reset)
export function IconReset(p: Props) {
  return (
    <IM
      {...p}
      paths={[
        "M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8",
        "M21 3v5h-5",
        "M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16",
        "M3 21v-5h5",
      ]}
    />
  );
}

// Printer
export function IconPrint(p: Props) {
  return (
    <IM
      {...p}
      paths={[
        "M6 18H4a2 2 0 0 1-2-2v-5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2v5a2 2 0 0 1-2 2h-2",
        "M6 9V3a1 1 0 0 1 1-1h10a1 1 0 0 1 1 1v6",
        "M6 14h12v8H6z",
      ]}
    />
  );
}

// Calendar (history)
export function IconCalendar(p: Props) {
  return (
    <IM
      {...p}
      paths={[
        "M8 2v4",
        "M16 2v4",
        "M3 10h18",
        "M5 4h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2z",
      ]}
    />
  );
}

// Sun
export function IconSun(p: Props) {
  return (
    <IM
      {...p}
      paths={[
        "M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8z",
        "M12 2v2",
        "M12 20v2",
        "m4.93 4.93 1.41 1.41",
        "m17.66 17.66 1.41 1.41",
        "M2 12h2",
        "M20 12h2",
        "m6.34 17.66-1.41 1.41",
        "m19.07 4.93-1.41 1.41",
      ]}
    />
  );
}

// Moon
export function IconMoon(p: Props) {
  return <I {...p} d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />;
}

// Sun+Moon (auto theme)
export function IconSunMoon(p: Props) {
  return (
    <IM
      {...p}
      paths={[
        "M12 2v2",
        "M14.837 16.385a6 6 0 1 1-7.223-7.222c.624-.147.97.66.715 1.248a4 4 0 0 0 5.26 5.259c.589-.255 1.396.09 1.248.715",
        "M16 12a4 4 0 0 0-4-4",
        "m19 5-1.256 1.256",
        "M20 12h2",
      ]}
    />
  );
}

// Help circle
export function IconHelp(p: Props) {
  return (
    <IM
      {...p}
      paths={[
        "M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20z",
        "M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3",
        "M12 17h.01",
      ]}
    />
  );
}

// Share
export function IconShare(p: Props) {
  return (
    <IM {...p} paths={["M4 12v8a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-8", "m16 6-4-4-4 4", "M12 2v13"]} />
  );
}

export function IconDot({ size, class: cls }: Props) {
  const s = size ?? defaults.size;
  return (
    <svg xmlns="http://www.w3.org/2000/svg" width={s} height={s} viewBox="0 0 24 24" class={cls}>
      <circle cx="12" cy="12" r="5" fill="currentColor" />
    </svg>
  );
}
