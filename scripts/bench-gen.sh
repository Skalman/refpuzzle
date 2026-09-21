#!/usr/bin/env bash
# Runs the release generator N times against year 2050 and tallies timing stats.
# Ctrl+C prints stats for whatever runs completed so far, then exits.
set -uo pipefail

n="${1:-10}"
bin="target/release/refpuzzle"
times=()

print_stats() {
  local count=${#times[@]}
  echo
  if [ "$count" -eq 0 ]; then
    echo "No completed runs."
    return
  fi
  printf '%s\n' "${times[@]}" | awk '
    { a[++n] = $1; sum += $1 }
    END {
      asort(a)
      mean = sum/n
      med = (n % 2) ? a[(n+1)/2] : (a[n/2] + a[n/2+1]) / 2
      for (i = 1; i <= n; i++) ss += (a[i] - mean) ^ 2
      sd = (n > 1) ? sqrt(ss / (n - 1)) : 0
      printf "n=%d  min=%.3f  mean=%.3f  median=%.3f  max=%.3f  std=%.3f  (ms/day)\n", n, a[1], mean, med, a[n], sd
    }
  '
}

trap 'print_stats; exit 130' INT

cargo build --release -q
for ((i = 1; i <= n; i++)); do
  line=$("$bin" gen 2050 -o /dev/null --stats -a 1000 2>&1 | grep 'Time:')
  if [[ $line =~ \(([0-9.]+)ms\ per\ day\) ]]; then
    echo "$line"
    times+=("${BASH_REMATCH[1]}")
  else
    echo "  run $i: FAILED (no Time: line)"
  fi
done

print_stats
