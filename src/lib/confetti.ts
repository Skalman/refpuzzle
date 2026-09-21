const COLORS = ["#6366f1", "#818cf8", "#22c55e", "#f59e0b", "#ef4444", "#60a5fa"];
const COUNT = 70;
const DURATION = 1800;
const GRAVITY = 900;

/**
 * Reduced motion: how much of the flight the long exposure covers, and how
 * many steps the streak is drawn in. Shorter than the full run, or the paths
 * leave the viewport before they arc.
 */
const TRACE_MS = 700;
const TRACE_STEPS = 44;
/** The fades the exposure arrives and leaves on, inside its run. */
const FADE_IN_MS = 200;
const FADE_OUT_MS = 500;

interface Particle {
  x: number;
  y: number;
  vx: number;
  vy: number;
  size: number;
  color: string;
  rotation: number;
  rotationSpeed: number;
}

/** The burst, from a point at `cx`/`cy`. */
function makeParticles(cx: number, cy: number): Particle[] {
  const particles: Particle[] = [];
  for (let i = 0; i < COUNT; i++) {
    const angle = Math.random() * Math.PI * 2;
    const speed = 200 + Math.random() * 400;
    particles.push({
      x: cx,
      y: cy,
      vx: Math.cos(angle) * speed,
      vy: Math.sin(angle) * speed - 250,
      size: 4 + Math.random() * 4,
      color: COLORS[Math.floor(Math.random() * COLORS.length)],
      rotation: Math.random() * Math.PI * 2,
      rotationSpeed: (Math.random() - 0.5) * 10,
    });
  }
  return particles;
}

/**
 * The burst as a long exposure: one streak per piece, faint where the exposure
 * opens and solid where the piece ends up, with the piece itself drawn at that
 * end. Painted once, into a buffer the frames then fade.
 */
function paintTrace(ctx: CanvasRenderingContext2D, particles: Particle[]) {
  const dt = TRACE_MS / 1000 / TRACE_STEPS;
  ctx.lineCap = "round";
  for (const p of particles) {
    let { x, y, vy, rotation } = p;
    ctx.strokeStyle = p.color;
    ctx.lineWidth = p.size * 0.5;
    for (let step = 1; step <= TRACE_STEPS; step++) {
      vy += GRAVITY * dt;
      const nextX = x + p.vx * dt;
      const nextY = y + vy * dt;
      rotation += p.rotationSpeed * dt;
      ctx.globalAlpha = 0.12 + 0.88 * (step / TRACE_STEPS);
      ctx.beginPath();
      ctx.moveTo(x, y);
      ctx.lineTo(nextX, nextY);
      ctx.stroke();
      x = nextX;
      y = nextY;
    }
    ctx.globalAlpha = 1;
    ctx.save();
    ctx.translate(x, y);
    ctx.rotate(rotation);
    ctx.fillStyle = p.color;
    ctx.fillRect(-p.size / 2, -p.size / 2, p.size, p.size * 0.6);
    ctx.restore();
  }
  ctx.globalAlpha = 1;
}

/**
 * Bursts confetti over the whole viewport. The canvas is a manual popover, so
 * it enters the top layer above any dialog open at the time. Under reduced
 * motion the same burst is drawn as a long exposure — the flight paths as
 * streaks, held still behind the dialog, arriving and leaving on a fade.
 */
export function confetti() {
  const calm = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const canvas = document.createElement("canvas");
  canvas.style.cssText =
    "position:fixed;inset:0;width:100%;height:100%;margin:0;padding:0;border:0;" +
    "background:transparent;overflow:visible;pointer-events:none;z-index:9999";
  document.body.appendChild(canvas);
  // The top layer carries the burst over an open dialog. The exposure stays
  // out of it, reading through the backdrop behind the dialog instead.
  if (!calm && "showPopover" in canvas) {
    canvas.popover = "manual";
    canvas.showPopover();
  }
  const ctx = canvas.getContext("2d")!;

  function resize() {
    canvas.width = window.innerWidth;
    canvas.height = window.innerHeight;
  }
  resize();

  const cx = canvas.width / 2;
  const cy = canvas.height * 0.4;
  const particles = makeParticles(cx, cy);

  let trace: HTMLCanvasElement | null = null;
  if (calm) {
    trace = document.createElement("canvas");
    trace.width = canvas.width;
    trace.height = canvas.height;
    paintTrace(trace.getContext("2d")!, particles);
  }

  const start = performance.now();
  let frame: number;

  function tick(now: number) {
    const elapsed = now - start;
    if (elapsed > DURATION) {
      canvas.remove();
      return;
    }

    ctx.clearRect(0, 0, canvas.width, canvas.height);

    if (trace) {
      const left = DURATION - elapsed;
      ctx.globalAlpha = Math.min(1, elapsed / FADE_IN_MS, Math.max(0, left / FADE_OUT_MS));
      ctx.drawImage(trace, 0, 0);
      frame = requestAnimationFrame(tick);
      return;
    }

    const dt = 1 / 60;
    ctx.globalAlpha = Math.max(0, 1 - elapsed / DURATION);

    for (const p of particles) {
      p.vy += GRAVITY * dt;
      p.x += p.vx * dt;
      p.y += p.vy * dt;
      p.rotation += p.rotationSpeed * dt;

      ctx.save();
      ctx.translate(p.x, p.y);
      ctx.rotate(p.rotation);
      ctx.fillStyle = p.color;
      ctx.fillRect(-p.size / 2, -p.size / 2, p.size, p.size * 0.6);
      ctx.restore();
    }

    frame = requestAnimationFrame(tick);
  }

  frame = requestAnimationFrame(tick);
  return () => {
    cancelAnimationFrame(frame);
    canvas.remove();
  };
}
