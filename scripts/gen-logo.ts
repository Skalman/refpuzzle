import { writeFileSync } from "node:fs";
import { resolve } from "node:path";

function vlen(x: number, y: number) {
  return Math.sqrt(x * x + y * y);
}

const nodes = {
  a: { x: 22, y: 28 },
  b: { x: 75, y: 22 },
  c: { x: 68, y: 75 },
  d: { x: 24, y: 78 },
};

const edges: [keyof typeof nodes, keyof typeof nodes][] = [
  ["a", "b"],
  ["b", "c"],
  ["c", "a"],
  ["d", "a"],
];

const DOT_R = 5;
const GAP = 8;
const HEAD_LEN = 15;
const HEAD_HALF_W = 6.5;
const ARC_R = 60;
const STROKE_W = 3;

// Find BOTH circle centers for an arc of radius R between two points
function arcCenters(sx: number, sy: number, ex: number, ey: number, r: number) {
  const mx = (sx + ex) / 2;
  const my = (sy + ey) / 2;
  const dx = ex - sx;
  const dy = ey - sy;
  const d = vlen(dx, dy);
  const h = Math.sqrt(Math.max(0, r * r - (d / 2) ** 2));
  const px = -dy / d;
  const py = dx / d;
  return {
    c1: { cx: mx + h * px, cy: my + h * py },
    c2: { cx: mx - h * px, cy: my - h * py },
  };
}

// Determine which center SVG uses for sweep=1, large-arc=0
// sweep=1 means: center is such that going from start to end in the
// positive-angle direction (CCW in math / CW in y-down) gives the short arc.
// Equivalent: cross product (end-start) × (center-start) > 0 for sweep=1 in y-down
function sweep1CenterOf(sx: number, sy: number, ex: number, ey: number, r: number) {
  const { c1, c2 } = arcCenters(sx, sy, ex, ey, r);
  const dx = ex - sx;
  const dy = ey - sy;
  // Cross product: (end-start) × (center-start)
  const cross1 = dx * (c1.cy - sy) - dy * (c1.cx - sx);
  // sweep=1 picks the center where this cross product is positive
  return cross1 > 0 ? c1 : c2;
}

// Determine the sweep flag needed to make SVG pick a specific center
function sweepFlagForCenter(
  sx: number,
  sy: number,
  ex: number,
  ey: number,
  cx: number,
  cy: number,
): 0 | 1 {
  const dx = ex - sx;
  const dy = ey - sy;
  const cross = dx * (cy - sy) - dy * (cx - sx);
  return cross > 0 ? 1 : 0;
}

function r(n: number): string {
  return n.toFixed(1);
}

const arcs: string[] = [];
const heads: string[] = [];
const debug: string[] = [];
const tipMids: { x: number; y: number; sweepFlag: number }[] = [];

for (const [fromKey, toKey] of edges) {
  const from = nodes[fromKey];
  const to = nodes[toKey];

  // Step 1: find the circle that SVG uses for sweep=1 arc from source→target
  const cc = sweep1CenterOf(from.x, from.y, to.x, to.y, ARC_R);

  // Step 2: find target angle on this circle, walk back to get stop points
  const targetAngle = Math.atan2(to.y - cc.cy, to.x - cc.cx);
  const srcAngle = Math.atan2(from.y - cc.cy, from.x - cc.cx);

  // Determine arc direction: which way does the sweep=1 arc go?
  // Try both +/- and see which gives a short arc
  let delta = targetAngle - srcAngle;
  // Normalize to find the short arc direction
  while (delta > Math.PI) delta -= 2 * Math.PI;
  while (delta < -Math.PI) delta += 2 * Math.PI;
  // The sign of delta tells us the direction: negative = CW (angle decreasing)
  const sign = delta < 0 ? -1 : 1; // direction of travel along arc

  // Walk back from target by (DOT_R + GAP + HEAD_LEN) for arc endpoint
  const totalBack = DOT_R + GAP + HEAD_LEN;
  const baseAngle = targetAngle - sign * (totalBack / ARC_R);
  const baseX = cc.cx + ARC_R * Math.cos(baseAngle);
  const baseY = cc.cy + ARC_R * Math.sin(baseAngle);

  // Walk back from target by (DOT_R + GAP) for arrowhead tip
  const tipBack = DOT_R + GAP;
  const tipAngle = targetAngle - sign * (tipBack / ARC_R);
  const tipX = cc.cx + ARC_R * Math.cos(tipAngle);
  const tipY = cc.cy + ARC_R * Math.sin(tipAngle);

  // Tangent at tip: perpendicular to radius, in the direction of travel
  const rx = tipX - cc.cx;
  const ry = tipY - cc.cy;
  // Direction of travel = sign * (perpendicular to radius)
  // In standard coords, CCW perp of (rx,ry) = (-ry, rx). CW = (ry, -rx).
  // If sign > 0 (CCW/increasing angle), tangent = (-ry, rx)
  // If sign < 0 (CW/decreasing angle), tangent = (ry, -rx)
  const tx = sign > 0 ? -ry / ARC_R : ry / ARC_R;
  const ty = sign > 0 ? rx / ARC_R : -rx / ARC_R;

  // Determine correct sweep flag for source→basePoint on the SAME circle
  const baseSweep = sweepFlagForCenter(from.x, from.y, baseX, baseY, cc.cx, cc.cy);

  // White arc from source to base
  arcs.push(
    `    <path d="M${r(from.x)},${r(from.y)} A${ARC_R},${ARC_R} 0 0,${baseSweep} ${r(baseX)},${r(baseY)}"/>`,
  );

  // Debug: red arc to target center (always sweep=1 as original)
  debug.push(
    `    <path d="M${r(from.x)},${r(from.y)} A${ARC_R},${ARC_R} 0 0,1 ${r(to.x)},${r(to.y)}" stroke="red" stroke-width="1.5" opacity="0.5"/>`,
  );
  // Debug: green dot at computed tip
  debug.push(`    <circle cx="${r(tipX)}" cy="${r(tipY)}" r="2" fill="lime" opacity="0.8"/>`);

  // Arrowhead: use chord direction (base→tip) for symmetry
  const chordX = tipX - baseX;
  const chordY = tipY - baseY;
  const cl = vlen(chordX, chordY);
  const cnx = chordX / cl;
  const cny = chordY / cl;
  const b1x = baseX + cny * HEAD_HALF_W;
  const b1y = baseY - cnx * HEAD_HALF_W;
  const b2x = baseX - cny * HEAD_HALF_W;
  const b2y = baseY + cnx * HEAD_HALF_W;
  heads.push(
    `    <polygon points="${r(tipX)},${r(tipY)} ${r(b1x)},${r(b1y)} ${r(b2x)},${r(b2y)}"/>`,
  );
  // Midpoint on the arc (between tip and base) for circle variant
  const midAngle = (tipAngle + baseAngle) / 2;
  const midX = cc.cx + ARC_R * Math.cos(midAngle);
  const midY = cc.cy + ARC_R * Math.sin(midAngle);
  tipMids.push({ x: midX, y: midY, sweepFlag: baseSweep });

  const deg = (Math.atan2(ty, tx) * 180) / Math.PI;
  console.log(
    `${fromKey}→${toKey}: sweep=${baseSweep} dir=${sign > 0 ? "CCW" : "CW"} tangent=${r(deg)}°`,
  );
}

const STYLE = `<style>@media (max-width: 47px) { .heads { display: none; } .arcs { stroke-width: 4.5; } }</style>`;

const nodeColors = ["#ff4d94", "#00d4ff", "#ffe014", "#00e676"]; // pink, blue, yellow, green
const arcColors = ["#ff4d94", "#00d4ff", "#ffe014", "#00e676"];
const nodeList = Object.values(nodes);

// Helper to wrap content in SVG
function makeSvg(content: string): string {
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width="100" height="100">\n${STYLE}\n${content}\n</svg>\n`;
}

// Timing constants
const ARC_DELAYS = [0, 0.15, 0.3, 0.45];
const HEAD_DELAYS = [0.25, 0.4, 0.55, 0.7];
const DOT_DELAYS = [0, 0.5, 1.0, 1.5];

// SVG builders with --d variable for timing
function buildArcs(color?: string[]): string {
  return arcs
    .map((a, i) => {
      let s = a.replace("<path", `<path style="--d:${ARC_DELAYS[i]}s"`);
      if (color) s = s.replace("/>", ` stroke="${color[i]}"/>`);
      return s;
    })
    .join("\n");
}

function buildHeads(color?: string[]): string {
  return heads
    .map((h, i) => {
      const mid = tipMids[i];
      let s = h.replace(
        "<polygon",
        `<polygon style="--d:${HEAD_DELAYS[i]}s; transform-origin:${r(mid.x)}px ${r(mid.y)}px"`,
      );
      if (color) s = s.replace("/>", ` fill="${color[i]}"/>`);
      return s;
    })
    .join("\n");
}

function buildDots(color?: string[]): string {
  return nodeList
    .map((n, i) => {
      const fill = color ? ` fill="${color[i]}"` : "";
      return `    <circle cx="${n.x}" cy="${n.y}" r="${DOT_R}"${fill} style="--d:${DOT_DELAYS[i]}s; transform-origin:${n.x}px ${n.y}px"/>`;
    })
    .join("\n");
}

// 11. Bold with circle tips — arrowheads become circles at small size
const tipCirclesSvg = tipMids
  .map((p) => `    <circle cx="${r(p.x)}" cy="${r(p.y)}" r="${DOT_R}"/>`)
  .join("\n");
const extendedArcsSvg = tipMids
  .map((p, i) => {
    const from =
      Object.values(nodes)[
        edges[i][0] === "a" ? 0 : edges[i][0] === "b" ? 1 : edges[i][0] === "c" ? 2 : 3
      ];
    return `    <path d="M${r(from.x)},${r(from.y)} A${ARC_R},${ARC_R} 0 0,${p.sweepFlag} ${r(p.x)},${r(p.y)}"/>`;
  })
  .join("\n");
// 12. Combined: animated draw + colored → white + pulsing
const v12 = makeSvg(`  <style>
    .tip-circles { display: none; }
    .arcs-ext { display: none; }
    @media (max-width: 47px) { .heads { display: none; } .arcs { display: none; } .arcs-ext { display: block; stroke-width: 8; } .tip-circles { display: block; } .dots circle { fill: white; animation: none; } }
    .arcs path { stroke-dasharray: 80; stroke-dashoffset: 80;
      animation: draw 1.0s ease-out var(--d) forwards, towhite-stroke 0.7s ease 0.8s forwards; }
    .heads polygon { opacity: 0; transform: scale(0);
      animation: pophead 0.25s ease-out var(--d) forwards, towhite-fill 0.7s ease 0.8s forwards; }
    .dots circle { animation: pulse 2s ease-in-out var(--d) 2, towhite-fill 0.7s ease 0.8s forwards; }
    @keyframes draw { to { stroke-dashoffset: 0; } }
    @keyframes pophead { to { opacity: 1; transform: scale(1); } }
    @keyframes pulse { 0%,100% { transform: scale(1); } 50% { transform: scale(1.4); } }
    .dots.once circle { animation: pulse 2s ease-in-out 1; animation-delay: var(--d); fill: white; }
    .dots.no-anim circle { animation: none; }
    @keyframes towhite-stroke { to { stroke: white; } }
    @keyframes towhite-fill { to { fill: white; } }
  </style>
  <script>
    document.addEventListener("DOMContentLoaded", function() {
      var running = true;
      var dotsG = document.querySelector(".dots");
      var lastDot = dotsG.querySelector("circle:last-child");
      lastDot.addEventListener("animationend", function(e) {
        if (e.animationName === "pulse") running = false;
      });
      function restart() {
        if (running) return;
        running = true;
        dotsG.classList.add("no-anim", "once");
        dotsG.getBBox();
        dotsG.classList.remove("no-anim");
      }
      document.documentElement.addEventListener("mouseenter", restart);
      document.documentElement.addEventListener("click", restart);
    });
  </script>
  <rect width="100" height="100" rx="20" fill="#6366f1"/>
  <g stroke-width="${STROKE_W}" stroke-linecap="round" fill="none" class="arcs">
${buildArcs(arcColors)}
  </g>
  <g stroke="white" stroke-width="${STROKE_W}" stroke-linecap="round" fill="none" class="arcs-ext">
${extendedArcsSvg}
  </g>
  <g class="heads">
${buildHeads(arcColors)}
  </g>
  <g fill="white" class="tip-circles">
${tipCirclesSvg}
  </g>
  <g class="dots">
${buildDots(nodeColors)}
  </g>`);

writeFileSync(resolve(import.meta.dirname, "../public/logo.svg"), v12);
console.log("Wrote logo.svg");
