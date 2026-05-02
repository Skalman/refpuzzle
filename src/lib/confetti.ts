const COLORS = ["#6366f1", "#818cf8", "#22c55e", "#f59e0b", "#ef4444", "#60a5fa"];
const COUNT = 70;
const DURATION = 1800;
const GRAVITY = 900;

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

export function confetti() {
  const canvas = document.createElement("canvas");
  canvas.style.cssText =
    "position:fixed;inset:0;width:100%;height:100%;pointer-events:none;z-index:9999";
  document.body.appendChild(canvas);
  const ctx = canvas.getContext("2d")!;

  function resize() {
    canvas.width = window.innerWidth;
    canvas.height = window.innerHeight;
  }
  resize();

  const cx = canvas.width / 2;
  const cy = canvas.height * 0.4;

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

  const start = performance.now();
  let frame: number;

  function tick(now: number) {
    const elapsed = now - start;
    if (elapsed > DURATION) {
      canvas.remove();
      return;
    }

    const dt = 1 / 60;
    const fade = Math.max(0, 1 - elapsed / DURATION);

    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.globalAlpha = fade;

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
