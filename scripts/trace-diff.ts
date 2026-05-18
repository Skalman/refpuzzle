import { execSync } from "node:child_process";

const args = process.argv.slice(2);
let dateRange: string | null = null;
let level: string | null = null;

for (let i = 0; i < args.length; i++) {
  if (args[i] === "-l" || args[i] === "--level") {
    level = args[++i];
  } else if (!args[i].startsWith("-")) {
    dateRange = args[i];
  }
}

if (!dateRange || !level) {
  console.error("Usage: pnpm trace-diff <date> -l <level>");
  console.error("Example: pnpm trace-diff 2051-01-01 -l 6");
  process.exit(1);
}

const tsCmd = `pnpm --silent generate ${dateRange} -l ${level} --trace -o /dev/null`;
const rsCmd = `cargo run --release -- gen ${dateRange} -l ${level} --trace -o /dev/null`;

function run(cmd: string): string[] {
  return execSync(`${cmd} 2>&1`, { encoding: "utf8" })
    .split("\n")
    .filter((l) => l.startsWith("{"));
}

function normalize(line: string): string {
  const obj = JSON.parse(line);
  return JSON.stringify(obj, Object.keys(obj).sort());
}

interface Action {
  qi: number;
  oi?: number;
  answer?: string;
  qm?: number;
  om?: number;
  rule: string;
}

function applyActions(
  actions: Action[],
  answers: (string | null)[],
  eliminated: number[],
) {
  for (const a of actions) {
    if (a.answer != null) {
      const oi = "ABCDE".indexOf(a.answer);
      answers[a.qi] = a.answer;
      eliminated[a.qi] = 0b11111 ^ (1 << oi);
    } else if (a.oi != null) {
      eliminated[a.qi] |= 1 << a.oi;
    } else if (a.qm != null && a.om != null) {
      for (let i = 0; i < 16; i++) {
        if ((a.qm >> i) & 1) eliminated[i] |= a.om;
      }
    }
  }
}

function formatState(answers: (string | null)[], eliminated: number[], n: number): string[] {
  const state: string[] = [];
  for (let i = 0; i < n; i++) {
    let s = "";
    if (answers[i] != null) {
      s = answers[i]!;
    } else {
      for (let oi = 0; oi < 5; oi++) {
        if ((eliminated[i] >> oi) & 1) s += "abcde"[oi];
      }
    }
    state.push(s);
  }
  return state;
}

const tsLines = run(tsCmd);
const rsLines = run(rsCmd);

// Find shared prefix
let i = 0;
while (
  i < tsLines.length &&
  i < rsLines.length &&
  normalize(tsLines[i]) === normalize(rsLines[i])
) {
  i++;
}

if (i >= tsLines.length && i >= rsLines.length) {
  console.log(`=== IDENTICAL (${String(i)} lines) ===`);
  process.exit(0);
}

// Replay shared prefix to reconstruct state
const shared = tsLines.slice(0, i);
let n = 0;
const puzzle: { qi: number; type: string; options: (number | null)[] }[] = [];
const answers: (string | null)[] = new Array(16).fill(null);
const eliminated: number[] = new Array(16).fill(0);

for (const line of shared) {
  const obj = JSON.parse(line);
  if (obj.t === "question") {
    puzzle.push({ qi: obj.qi, type: obj.type, options: obj.options });
    n = Math.max(n, obj.qi + 1);
  } else if (obj.t === "batch") {
    applyActions(obj.actions, answers, eliminated);
  } else if (obj.t === "lookahead") {
    const [eqi, eoi] = obj.eliminate;
    eliminated[eqi] |= 1 << eoi;
  }
}

// Print shared trace (compact)
for (const line of shared) {
  const obj = JSON.parse(line);
  if (obj.t === "phase" || obj.t === "construct_failed") continue;
  console.log(line);
}

// Print state at divergence
console.log(`\n=== DIVERGENCE at line ${String(i + 1)} ===`);
console.log(`\n--- State at divergence (n=${String(n)}) ---`);

const state = formatState(answers, eliminated, n);
console.log(JSON.stringify({ puzzle, state }));

// Print diverging lines
if (i < tsLines.length) {
  console.log("\n--- TypeScript ---");
  for (let j = i; j < Math.min(i + 15, tsLines.length); j++) console.log(tsLines[j]);
  if (tsLines.length > i + 15)
    console.log(`... (${String(tsLines.length - i - 15)} more lines)`);
}

if (i < rsLines.length) {
  console.log("\n--- Rust ---");
  for (let j = i; j < Math.min(i + 15, rsLines.length); j++) console.log(rsLines[j]);
  if (rsLines.length > i + 15)
    console.log(`... (${String(rsLines.length - i - 15)} more lines)`);
}

process.exit(1);
