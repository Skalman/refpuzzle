import { useLayoutEffect, useEffect, useRef, useState } from "preact/hooks";
import type { Marks } from "../engine/types.ts";
import { LETTERS } from "../engine/types.ts";
import type { CoachMessage } from "../engine/coach-types.ts";

interface Props {
  message: CoachMessage | null;
  gridRef: { current: HTMLDivElement | null };
  textRef: { current: HTMLDivElement | null };
  marks: Marks[];
  optionCount: number;
}

interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}
type Shape =
  | { t: "halo"; rect: Rect }
  | { t: "line"; x1: number; y1: number; x2: number; y2: number; head: boolean }
  | { t: "boundary"; x1: number; y1: number; x2: number };
interface Tally {
  x: number;
  y: number;
  counts: { letter: string; n: number }[];
}
interface Geom {
  shapes: Shape[];
  tally: Tally | null;
}

const PAD = 4;
// Shortest a pointer line may be, so a close target (e.g. the top row, right
// under the text) isn't a stubby arc with an oversized head.
const MIN_POINTER = 40;

/**
 * SVG overlay for the L1 coach's arrows: soft halos + connector lines drawn
 * over the board, plus a per-letter tally badge for whole-grid question kinds.
 * Purely presentational and `pointer-events: none`; measures the live board
 * geometry so it tracks layout/scroll. Draw-on + pulse are CSS, gated to
 * `prefers-reduced-motion: no-preference`.
 */
export function CoachArrows({ message, gridRef, textRef, marks, optionCount }: Props) {
  const [svg, setSvg] = useState<SVGSVGElement | null>(null);
  const [geom, setGeom] = useState<(Geom & { seq: number }) | null>(null);
  const [viewport, setViewport] = useState(0);

  // `seq` keys the shapes. It bumps on a genuinely new message (→ remount →
  // replay draw-on), but NOT when a viewport/board recompute leaves the message
  // unchanged, nor when consecutive messages share an `arrowKey` (a mistake flag
  // and its escalation) — those keep the elements so the arrow transitions.
  const seqRef = useRef(0);
  const lastMsgRef = useRef<CoachMessage | null>(null);
  const lastKeyRef = useRef<string | null>(null);

  // A signature of the current answers: recompute geometry (and tally counts)
  // when the board or the message changes, or the viewport shifts.
  const answerSig = marks.map((m) => m.indexOf("correct")).join(",");

  useLayoutEffect(() => {
    const g = computeGeometry({
      svg,
      grid: gridRef.current,
      text: textRef.current,
      message,
      marks,
      optionCount,
    });
    const key = message?.arrowKey ?? null;
    const sharesKey = key != null && key === lastKeyRef.current;
    if (message !== lastMsgRef.current && !sharesKey) seqRef.current += 1;
    lastMsgRef.current = message;
    lastKeyRef.current = key;
    setGeom(g ? { ...g, seq: seqRef.current } : null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [svg, message, answerSig, viewport]);

  // Only resize can reflow the board and change the arrows' geometry. Scroll
  // can't: the overlay and the rows share `.puzzle-view`, so their relative
  // (svg-local) positions are scroll-invariant — no recompute needed.
  useEffect(() => {
    const onResize = () => setViewport((v) => v + 1);
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);

  return (
    <div class="coach-overlay" aria-hidden="true">
      <svg ref={setSvg} class="coach-svg">
        {geom?.shapes.map((s, i) => renderShape(s, i, geom.seq))}
      </svg>
      {geom?.tally && (
        <div class="coach-tally" style={{ left: `${geom.tally.x}px`, top: `${geom.tally.y}px` }}>
          {geom.tally.counts.map((c) => (
            <span key={c.letter} class="coach-tally-item">
              <span class="coach-tally-letter">{c.letter}</span>
              {c.n}
            </span>
          ))}
        </div>
      )}
    </div>
  );
}

function renderShape(s: Shape, i: number, seq: number) {
  // Key by seq so shapes remount on a new message (replaying the draw-on),
  // but stay mounted across viewport/board recomputes of the same message.
  const key = `${seq}-${i}`;
  if (s.t === "halo") {
    return (
      <rect
        key={key}
        class="coach-halo"
        x={s.rect.x}
        y={s.rect.y}
        width={s.rect.w}
        height={s.rect.h}
        rx="8"
      />
    );
  }
  if (s.t === "boundary") {
    return <line key={key} class="coach-boundary" x1={s.x1} y1={s.y1} x2={s.x2} y2={s.y1} />;
  }
  // Slight arc (quadratic bézier). `pathLength` normalizes the draw-on dash so
  // it covers the whole curve regardless of length. A wider under-stroke gives a
  // crisp outline (light mode); both are themed via coach.css.
  const d = arcPath(s.x1, s.y1, s.x2, s.y2, s.head);
  return (
    <g key={key}>
      <path class="coach-line-outline" d={d} pathLength={100} fill="none" />
      <path class="coach-line-arrow" d={d} pathLength={100} fill="none" />
    </g>
  );
}

/**
 * A quadratic bézier from (x1,y1) to (x2,y2) that bows slightly to one side — a
 * soft arc rather than a straight line. When `head`, the arrowhead is appended
 * to the same path so the draw-on dash reveals it last, rather than an SVG
 * marker that would pop in immediately.
 */
function arcPath(x1: number, y1: number, x2: number, y2: number, head: boolean): string {
  const dx = x2 - x1;
  const dy = y2 - y1;
  const len = Math.hypot(dx, dy) || 1;
  // Bow grows faster than linearly with length, so short arrows stay nearly
  // straight (and their heads point straight) while long ones curve. Capped.
  const bow = Math.min(len * len * 0.001, 24);
  // Control point at the midpoint, offset along the (normalized) perpendicular.
  const cx = (x1 + x2) / 2 + (-dy / len) * bow;
  const cy = (y1 + y2) / 2 + (dx / len) * bow;
  let d = `M${x1},${y1} Q${cx},${cy} ${x2},${y2}`;
  if (head) {
    // Two barbs off the tip, along the curve's end tangent (control → tip).
    const tx = x2 - cx;
    const ty = y2 - cy;
    const tl = Math.hypot(tx, ty) || 1;
    const ux = tx / tl;
    const uy = ty / tl;
    const hl = 8;
    const b1x = x2 - hl * ux - hl * 0.5 * uy;
    const b1y = y2 - hl * uy + hl * 0.5 * ux;
    const b2x = x2 - hl * ux + hl * 0.5 * uy;
    const b2y = y2 - hl * uy - hl * 0.5 * ux;
    d += ` L${b1x},${b1y} L${x2},${y2} L${b2x},${b2y}`;
  }
  return d;
}

function computeGeometry(ctx: {
  svg: SVGSVGElement | null;
  grid: HTMLDivElement | null;
  text: HTMLDivElement | null;
  message: CoachMessage | null;
  marks: Marks[];
  optionCount: number;
}): Geom | null {
  const { svg, grid, text, message, marks, optionCount } = ctx;
  if (!svg || !grid || !message || !message.arrow) return null;
  const arrow = message.arrow;
  const origin = svg.getBoundingClientRect();
  const rel = (el: Element | null): Rect | null => {
    if (!el) return null;
    const r = el.getBoundingClientRect();
    return { x: r.left - origin.left, y: r.top - origin.top, w: r.width, h: r.height };
  };
  const row = (qi: number) => rel(grid.querySelector(`.question-row[data-qi="${qi}"]`));
  const cell = (qi: number, oi: number) =>
    rel(grid.querySelector(`[data-qi="${qi}"][data-oi="${oi}"]`));
  const n = marks.length;

  const shapes: Shape[] = [];
  let tally: Tally | null = null;
  const textRect = rel(text);

  // A soft "look here" line from the coach text down to a target row. `frac`
  // spreads the endpoints across the text/row width so multiple pointers land at
  // distinct x and never lie on top of one another.
  const pointerLine = (r: Rect, frac: number) => {
    if (!textRect) return;
    const x2 = r.x + r.w * frac;
    const y2 = r.y;
    let x1 = textRect.x + textRect.w * frac;
    let y1 = textRect.y + textRect.h;
    const dx = x2 - x1;
    const dy = y2 - y1;
    const len = Math.hypot(dx, dy);
    if (len > 0 && len < MIN_POINTER) {
      // Pull the start back along the line so a close target still gets a full arrow.
      x1 = x2 - (dx / len) * MIN_POINTER;
      y1 = y2 - (dy / len) * MIN_POINTER;
    }
    shapes.push({ t: "line", x1, y1, x2, y2, head: true });
  };

  if (arrow.mode === "point") {
    // Halo every question the step reads (or the specific option cell when `oi`
    // is set). A lone target gets a pointer from the hint text; multiple targets
    // are linked to each other (#1 → #3) instead.
    const targets = arrow.qis
      .map((qi) => (arrow.oi != null ? cell(qi, arrow.oi) : row(qi)))
      .filter((r): r is Rect => r != null)
      .sort((a, b) => a.y - b.y);
    for (const r of targets) shapes.push({ t: "halo", rect: pad(r) });
    if (targets.length === 1) {
      pointerLine(targets[0], 0.5);
    } else {
      for (let i = 0; i + 1 < targets.length; i++) {
        shapes.push(edgeConnect(targets[i], targets[i + 1]));
      }
    }
    return { shapes, tally };
  }

  // Connector: the asking question's row + its referent ("this refers to that").
  const anchor = row(arrow.qi);
  if (anchor) {
    shapes.push({ t: "halo", rect: pad(anchor) });
    pointerLine(anchor, 0.5);
    const ref = arrow.referent;
    switch (ref.kind) {
      case "column": {
        for (let qi = 0; qi < n; qi++) {
          const b = ref.boundary;
          if (b && ((b.side < 0 && qi >= b.qi) || (b.side > 0 && qi <= b.qi))) continue;
          const c = cell(qi, ref.oi);
          if (c) shapes.push({ t: "halo", rect: pad(c) });
        }
        if (ref.boundary) {
          const br = row(ref.boundary.qi);
          if (br) {
            const y = ref.boundary.side < 0 ? br.y : br.y + br.h;
            shapes.push({ t: "boundary", x1: br.x, y1: y, x2: br.x + br.w });
          }
        }
        break;
      }
      case "question": {
        const r = row(ref.qi);
        if (r) {
          shapes.push({ t: "halo", rect: pad(r) });
          shapes.push(connect(anchor, r));
        }
        break;
      }
      case "scan": {
        const from = row(ref.qi);
        if (from) {
          shapes.push({ t: "halo", rect: pad(from) });
          shapes.push(scan(from, ref.dir));
          // Halo the hunted answer's option in the rows the scan covers, so it
          // shows *which* answer it's looking for, not just the direction.
          for (let qi = 0; qi < n; qi++) {
            if (ref.dir < 0 ? qi < ref.qi : qi > ref.qi) {
              const c = cell(qi, ref.oi);
              if (c) shapes.push({ t: "halo", rect: pad(c) });
            }
          }
        }
        break;
      }
      case "sameRun":
        shapes.push(scan(anchor, ref.dir));
        break;
      case "candidates":
        for (const cqi of ref.qis) {
          const r = row(cqi);
          if (r) {
            shapes.push({ t: "halo", rect: pad(r) });
            shapes.push(connect(anchor, r));
          }
        }
        break;
      case "tally": {
        const counts = LETTERS.slice(0, optionCount).map((letter, oi) => ({
          letter,
          n: marks.reduce((acc, m) => acc + (m[oi] === "correct" ? 1 : 0), 0),
        }));
        tally = { x: anchor.x + anchor.w + 8, y: anchor.y, counts };
        break;
      }
    }
  }

  return { shapes, tally };
}

function pad(r: Rect): Rect {
  return { x: r.x - PAD, y: r.y - PAD, w: r.w + 2 * PAD, h: r.h + 2 * PAD };
}

/** A connector line between the centers of two rects. */
function connect(a: Rect, b: Rect): Shape {
  return {
    t: "line",
    x1: a.x + a.w / 2,
    y1: a.y + a.h / 2,
    x2: b.x + b.w / 2,
    y2: b.y + b.h / 2,
    head: false,
  };
}

/**
 * An arrow from one row to another, anchored at their left edges so the arc
 * bows out into the margin (around any rows in between).
 */
function edgeConnect(a: Rect, b: Rect): Shape {
  return { t: "line", x1: a.x, y1: a.y + a.h / 2, x2: b.x, y2: b.y + b.h / 2, head: true };
}

/** A short vertical arrow leaving a row's near edge in `dir` (−1 up, +1 down). */
function scan(from: Rect, dir: number): Shape {
  const x = from.x + from.w / 2;
  const y = dir < 0 ? from.y : from.y + from.h;
  return { t: "line", x1: x, y1: y, x2: x, y2: y + dir * 28, head: true };
}
