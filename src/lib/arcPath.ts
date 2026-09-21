/**
 * A quadratic bézier from (x1,y1) to (x2,y2) that bows slightly to one side — a
 * soft arc rather than a straight line. When `head`, the arrowhead is appended
 * to the same path so a draw-on dash reveals it last, rather than an SVG
 * marker that would pop in immediately.
 */
export function arcPath(x1: number, y1: number, x2: number, y2: number, head: boolean): string {
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
