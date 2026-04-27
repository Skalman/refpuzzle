#!/usr/bin/env bash
set -euo pipefail

# Regenerate puzzle JSON files, preserving days before a cutoff date.
#
# Usage: scripts/regenerate.sh [--from YYYY-MM-DD] [--until YYYY]
#
# If --from is omitted, preserves all days up to yesterday (safe default).
# If --until is omitted, generates up to current year + 3 (plus any existing files beyond that).
# The Rust generator reproduces puzzles deterministically from date seeds,
# but option ordering may differ after code changes — so we preserve old
# days to avoid breaking existing share URLs and save states.

cd "$(dirname "$0")/.."

FROM=""
UNTIL=""
while [ $# -gt 0 ]; do
  case $1 in
    --from) FROM="$2"; shift 2 ;;
    --from=*) FROM="${1#*=}"; shift ;;
    --until) UNTIL="$2"; shift 2 ;;
    --until=*) UNTIL="${1#*=}"; shift ;;
    *) echo "Usage: $0 [--from YYYY-MM-DD] [--until YYYY]" >&2; exit 1 ;;
  esac
done

if [ -z "$FROM" ]; then
  FROM=$(date -d yesterday +%Y-%m-%d)
  echo "No --from specified, preserving days before $FROM"
fi

FROM_MMDD=$(echo "$FROM" | sed 's/-//g' | cut -c5-8)

echo "Building Rust generator..."
cargo build --release --manifest-path rust/Cargo.toml 2>&1 | tail -1

CURRENT_YEAR=$(date +%Y)
END_YEAR=$((CURRENT_YEAR + 3))
[ -n "$UNTIL" ] && END_YEAR="$UNTIL"

# Also cover any existing files beyond END_YEAR
for f in public/puzzles/daily/*.json; do
  y=$(basename "$f" .json)
  [ "$y" -gt "$END_YEAR" ] 2>/dev/null && END_YEAR="$y"
done

for year in $(seq 2026 "$END_YEAR"); do
  FILE="public/puzzles/daily/$year.json"
  TMP="/tmp/refpuzzle-regen-$year.json"

  echo "Generating $year..."
  cargo run --release --manifest-path rust/Cargo.toml -- --year "$year" 2>/dev/null > "$TMP"

  if [ -f "$FILE" ] && [ "$year" = "$(echo "$FROM" | cut -c1-4)" ]; then
    echo "  Preserving days before $FROM_MMDD in $year..."
    node --permission \
      --allow-fs-read="$PWD" --allow-fs-read="/tmp" \
      --allow-fs-write="$PWD/public" \
      --experimental-transform-types -e "
      import { readFileSync, writeFileSync } from 'node:fs';
      const oldData = JSON.parse(readFileSync('$FILE', 'utf8'));
      const newData = JSON.parse(readFileSync('$TMP', 'utf8'));
      for (const mmdd of Object.keys(oldData)) {
        if (mmdd < '$FROM_MMDD') newData[mmdd] = oldData[mmdd];
      }
      writeFileSync('$FILE', JSON.stringify(newData));
    " 2>/dev/null
  else
    cp "$TMP" "$FILE"
  fi

  rm -f "$TMP"
  echo "  Done: $(wc -c < "$FILE" | tr -d ' ') bytes"
done

echo ""
echo "All years regenerated. Days before $FROM preserved."
