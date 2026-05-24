import type { Answer } from "../engine/types.ts";
import type { DeduceAction } from "../engine/deduce.ts";
import type { LookaheadResult } from "../engine/lookahead.ts";
import type { RNG } from "./rng.ts";

function emit(obj: Record<string, unknown>) {
  console.error(JSON.stringify(obj));
}

export function traceConstructFailed(attempt: number) {
  emit({ t: "construct_failed", attempt });
}

export function traceAttempt(attempt: number, solution: Answer[], rng: RNG) {
  emit({ t: "attempt", attempt, solution: solution.join(""), rng: rng.state() });
}

export function traceQuestion(
  qi: number,
  type: string,
  options: (number | null)[],
  rng: RNG,
  claims?: unknown,
) {
  const obj: Record<string, unknown> = { t: "question", qi, type, options, rng: rng.state() };
  if (claims) obj.claims = claims;
  emit(obj);
}

export function tracePhase(name: string, placed: { qi: number; type: string }[]) {
  emit({ t: "phase", name, placed });
}

export function traceSuccess(attempt: number) {
  emit({ t: "success", attempt });
}

export function traceFailed(attempt: number) {
  emit({ t: "failed", attempt });
}

export function traceSolve(label: string) {
  emit({ t: "solve", label });
}

export function traceHint(solved: boolean, answered: number, n: number) {
  emit({ t: "hint", solved, answered, n });
}

export function traceUniqueness(solutions: number) {
  emit({ t: "uniqueness", solutions });
}

export function traceBatch(batch: number, results: { action: DeduceAction; rule: string }[]) {
  const actions = results.map((r) => formatDeduceAction(r.action, r.rule));
  emit({ t: "batch", batch, actions });
}

export function traceLookahead(lr: LookaheadResult) {
  emit({
    t: "lookahead",
    assume: [lr.assumptionQi, lr.assumptionAnswer],
    contradiction: lr.contradictionQi,
    eliminate: [lr.eliminateQi, lr.eliminateOi],
    chain: lr.chain.map((dr) => formatDeduceAction(dr.action, dr.rule)),
  });
}

export function traceRepair(
  qi: number,
  before: (number | null)[],
  after: (number | null)[],
  probe: number,
) {
  emit({ t: "repair", qi, before, after, probe });
}

export function traceRepairNoChange(qi: number) {
  emit({ t: "repair", qi, changed: false });
}

function formatDeduceAction(
  a: DeduceAction,
  rule: string,
): { qi: number; oi?: number; answer?: string; qm?: number; om?: number; rule: string } {
  if (a.type === "force") return { qi: a.qi, answer: a.answer, rule };
  if (a.type === "eliminate") return { qi: a.qi, oi: a.oi, rule };
  return { qi: -1, qm: a.questionMask, om: a.optionMask, rule };
}
