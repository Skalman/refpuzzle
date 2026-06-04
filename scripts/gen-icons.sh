#!/usr/bin/env bash
set -euo pipefail

# Regenerate the logo SVGs, then rasterize the PWA icon PNGs from them and
# optimize with oxipng. Run this after changing the logo design in gen-logo.ts.
#
#   icon-{192,512}.png          <- logo-static.svg   (purpose: any, rounded tile)
#   icon-maskable-{192,512}.png <- logo-maskable.svg (purpose: maskable, full-bleed)
#
# Requires: node, inkscape (>= 0.92), oxipng.

cd "$(dirname "$0")/.."
ROOT="$PWD"

echo "==> Generating logo SVGs (gen-logo.ts)"
node --permission \
  --allow-fs-read="$ROOT" \
  --allow-fs-write="$ROOT/public/logo.svg" \
  --allow-fs-write="$ROOT/src/assets/logo.svg" \
  --allow-fs-write="$ROOT/public/logo-static.svg" \
  --allow-fs-write="$ROOT/public/logo-maskable.svg" \
  scripts/gen-logo.ts

# Rasterize <svg> at <size>x<size> to <out>. Inkscape 0.92 CLI uses --export-png
# (1.x's --export-type/-o is NOT supported here).
render() {
  inkscape --without-gui --export-png="$3" --export-width="$2" --export-height="$2" "$1" \
    >/dev/null 2>&1
}

echo "==> Rasterizing icons (inkscape)"
render public/logo-static.svg 192 public/icon-192.png
render public/logo-static.svg 512 public/icon-512.png
render public/logo-maskable.svg 192 public/icon-maskable-192.png
render public/logo-maskable.svg 512 public/icon-maskable-512.png

echo "==> Optimizing (oxipng)"
oxipng -o4 --strip safe \
  public/icon-192.png public/icon-512.png \
  public/icon-maskable-192.png public/icon-maskable-512.png

echo "Done: icon-{192,512}.png + icon-maskable-{192,512}.png"
