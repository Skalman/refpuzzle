import { test } from "node:test";
import assert from "node:assert/strict";
import { encodeHistory, decodeHistory, migrateValue, isSolvedValue } from "./store.ts";

// The format these exercise is specified in docs/encoding.md. Strings here are
// the spec's own examples wherever one fits.

/** Decode then re-encode: the canonical form must be a fixed point. */
function roundTrip(encoded: string, n: number): string {
  const state = decodeHistory(encoded, n);
  assert.ok(state, `failed to decode ${encoded}`);
  return encodeHistory(state);
}

test("v1 round-trips a mid-solve string unchanged", () => {
  const encoded = "v1.3b.3c.7D.cp.2a.5C.h1.1A";
  assert.equal(roundTrip(encoded, 7), encoded);
});

test("v1 round-trips a cursor parked mid-history", () => {
  const encoded = "v1.3b.3c.7D.cp.2a._.5C.h1.1A";
  assert.equal(roundTrip(encoded, 7), encoded);
});

test("v1 round-trips a cursor on the blank board", () => {
  // The v0 quirk this fixes is pinned below: absent `_` is the only end-marker,
  // so a save made at the blank board survives.
  const encoded = "v1._.3b.4E";
  assert.equal(roundTrip(encoded, 4), encoded);

  const state = decodeHistory(encoded, 4);
  assert.equal(state?.historyIdx, 0);
});

test("v1 keeps annotations that lead the string, on step 0", () => {
  // A refusal before the first mark — recorded against Start.
  const encoded = "v1.cpx1.3b.4E";
  const state = decodeHistory(encoded, 4);
  assert.deepEqual(state?.fails.get(0), { count: 1, qis: [] });
  assert.equal(roundTrip(encoded, 4), encoded);
});

test("v1 emits h before cpx on the same step", () => {
  const encoded = "v1.3b.h2.cpx3";
  const state = decodeHistory(encoded, 3);
  assert.deepEqual(state?.hints.get(1), { level: 2, qi: null });
  assert.deepEqual(state?.fails.get(1), { count: 3, qis: [] });
  assert.equal(roundTrip(encoded, 3), encoded);
});

test("v1 round-trips the questions a marker names", () => {
  // `q` marks and separates: one question on the hint, two on the refusals.
  const encoded = "v1.3b.h2q4.cpx2q1q4";
  const state = decodeHistory(encoded, 4);
  assert.deepEqual(state?.hints.get(1), { level: 2, qi: 3 });
  assert.deepEqual(state?.fails.get(1), { count: 2, qis: [0, 3] });
  assert.equal(roundTrip(encoded, 4), encoded);
});

test("v1 reads a marker with no question as naming none", () => {
  // Pre-suffix strings stay valid; they simply name no question.
  const state = decodeHistory("v1.3b.h1.cpx1", 4);
  assert.equal(state?.hints.get(1)?.qi, null);
  assert.deepEqual(state?.fails.get(1)?.qis, []);
});

test("v1 skips an unknown token without pushing a step", () => {
  // Lossless for annotations: a newer dialect's marker costs a badge, not the board.
  const state = decodeHistory("v1.3b.zz9.4E", 4);
  assert.equal(state?.history.length, 3);
  assert.equal(state?.questions[2].marks[1], "incorrect");
  assert.equal(state?.questions[3].marks[4], "correct");
});

test("v1 skips an action naming a question the board doesn't have", () => {
  const state = decodeHistory("v1.9A", 3);
  assert.equal(state?.history.length, 1);
});

test("v1 carries no completion — that lives in the ledger", () => {
  const state = decodeHistory("v1.3b.4E", 4);
  assert.equal(state?.completed, false);
  assert.equal(state?.stale, false);
});

test("v0 decodes marks, and reads x/! by position", () => {
  const solved = decodeHistory("3b.4E.x", 4);
  assert.equal(solved?.completed, true);
  assert.equal(solved?.stale, false);

  const wentStale = decodeHistory("3b.4E.x.!", 4);
  assert.equal(wentStale?.completed, true);
  assert.equal(wentStale?.stale, true);
});

test("v0 loses a cursor parked on the blank board (known quirk)", () => {
  // Its cursor initializes to 0 and a leading `_` also yields 0, so the
  // default-to-end rule fires. Pinned as behavior, not endorsed.
  const state = decodeHistory("_.3b.4E", 4);
  assert.equal(state?.historyIdx, 2);
});

test("v0 skips an unknown token without pushing a phantom step", () => {
  const state = decodeHistory("3b.zz9.4E", 4);
  assert.equal(state?.history.length, 3);
});

test("v0 marks the current step from an action's _ prefix", () => {
  const state = decodeHistory("3b._4E.2a", 4);
  assert.equal(state?.historyIdx, 2);
});

test("migration rewrites the v0 unmark prefix to a v1 suffix", () => {
  assert.equal(migrateValue("3b.-3b"), "v1.3b.3b-");
});

test("migration splits a v0 cursor prefix into an action and a marker", () => {
  assert.equal(migrateValue("_3b.4E"), "v1.3b._.4E");
});

test("migration converts x/! into the outcome ledger", () => {
  assert.equal(migrateValue("3b.4E.x"), "v1.3b.4E|s");
  assert.equal(migrateValue("3b.4E.x.!"), "v1.3b.4E|s.st");
});

test("migration keeps the counters behind the solved flag", () => {
  assert.equal(migrateValue("3b.x|s2e743"), "v1.3b|s.se2.e743");
});

test("migration never emits st without s", () => {
  // A stale flag with no completion has nothing to qualify.
  assert.equal(migrateValue("3b.!"), "v1.3b");
});

test("migration rewrites the v0 scan-parsed meta as discrete tokens", () => {
  assert.equal(migrateValue("3b.4E|s2e743n1h2c1f"), "v1.3b.4E|se2.e743.n1.h2.cp1.f");
});

test("migration is idempotent and leaves v1 untouched", () => {
  const v1 = "v1.3b.4E|se2.e743";
  assert.equal(migrateValue(v1), v1);

  const once = migrateValue("3b.4E.x");
  assert.equal(migrateValue(once), once);
});

test("the solved check reads the ledger exactly, not by substring", () => {
  // `se2` opens with an `s`; only a discrete token counts.
  assert.equal(isSolvedValue("v1.3b|s"), true);
  assert.equal(isSolvedValue("v1.3b|s.st"), true);
  assert.equal(isSolvedValue("v1.3b|s.se2.e743.n1"), true);
  assert.equal(isSolvedValue("v1.3b|se2.e743"), false);
  assert.equal(isSolvedValue("v1.3b"), false);
});
