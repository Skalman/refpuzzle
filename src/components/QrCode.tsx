import { encode } from "uqr";

export default function renderQrSvg(data: string): string {
  const result = encode(data, { ecc: "L" });
  const qrSize = result.size;

  let d = "";
  for (let y = 0; y < qrSize; y++) {
    for (let x = 0; x < qrSize; x++) {
      if (result.data[y][x]) d += `M${x},${y}h1v1h-1z`;
    }
  }

  return `<svg viewBox="0 0 ${qrSize} ${qrSize}" shape-rendering="crispEdges"><rect width="${qrSize}" height="${qrSize}" fill="white"/><path d="${d}" fill="black"/></svg>`;
}
