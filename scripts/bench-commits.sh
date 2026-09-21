#!/usr/bin/env bash
# Times `gen` across a range of commits, building each one fresh.
#
#   bash scripts/bench-commits.sh <git range>      e.g. HEAD~10..HEAD
#
# Checks each commit out in turn, so it refuses to start on a dirty tree and
# puts the original branch back on the way out, interrupt included.
set -euo pipefail

range=${1:-}
if [ -z "$range" ]; then
  echo "usage: $0 <git range>   e.g. $0 HEAD~10..HEAD" >&2
  exit 2
fi

if ! git diff --quiet || ! git diff --cached --quiet; then
  echo "working tree has changes; commit or stash them first" >&2
  exit 1
fi

start=$(git rev-parse --abbrev-ref HEAD)
[ "$start" = "HEAD" ] && start=$(git rev-parse HEAD)
trap 'git checkout -q "$start"' EXIT

for commit in $(git log --reverse --format=%h "$range"); do
  git checkout -q "$commit"
  echo "=== $(git log --oneline -1) ==="
  if ! cargo build --release -q 2>/dev/null; then
    echo "  BUILD FAILED"
    continue
  fi
  for _ in {1..5}; do
    cargo run --release -q -- gen 2050 -o /dev/null --stats 2>&1 | grep Time || echo "  RUN FAILED"
  done
done
