import type { Answer } from "./types.ts";

/** Stable identifier for each deduce rule the engine knows about.
 * Mirrors the `DeduceRule` enum in `rust/src/deduce.rs`. */
export type DeduceRule =
  | "CountSaturated"
  | "CountMustMatchForce"
  | "CountMustMatchElim"
  | "OnlyOptionLeft"
  | "AnswerOfForward"
  | "AnswerOfReverse"
  | "SameAsReverse"
  | "PrevNextOnlySameReverse"
  | "LetterDistForward"
  | "LetterDistReverseForce"
  | "LetterDistReverseElim"
  | "CountAllAnswered"
  | "MostCommonCountElim"
  | "PositionalRangeAnswered"
  | "PositionalRangeUnanswered"
  | "VowelCrossElim"
  | "ConsonantCrossElim"
  | "CountExceeded"
  | "CountImpossible"
  | "AnswerOfTargetRuledOut"
  | "LetterDistImpossible"
  | "LetterDistWrong"
  | "LetterDistNoMatch"
  | "FirstClosestAfterOutOfRange"
  | "FirstClosestAfterWrongAnswer"
  | "FirstClosestAfterRuledOut"
  | "FirstClosestAfterEarlierMatch"
  | "FirstClosestAfterSelfRef"
  | "FirstClosestAfterNoneMatch"
  | "LastClosestBeforeOutOfRange"
  | "LastClosestBeforeWrongAnswer"
  | "LastClosestBeforeRuledOut"
  | "LastClosestBeforeLaterMatch"
  | "LastClosestBeforeSelfRef"
  | "LastClosestBeforeNoneMatch"
  | "OnlyOddEvenWrongParity"
  | "OnlyOddEvenWrongAnswer"
  | "OnlyOddEvenRuledOut"
  | "OnlyOddEvenNoneMatch"
  | "ConsecIdentOutOfRange"
  | "ConsecIdentSelfRef"
  | "ConsecIdentNoCommon"
  | "ConsecIdentNonePair"
  | "EqualCountSelfRef"
  | "PrevSameNotBefore"
  | "PrevSameRuledOut"
  | "PrevSameCloser"
  | "NextSameNotAfter"
  | "NextSameRuledOut"
  | "NextSameCloser"
  | "OnlySameSelfRef"
  | "OnlySameRuledOut"
  | "UniqueAlreadyUsed"
  | "LeastCommonElim"
  | "LeastCommonForce"
  | "LeastCommonCountFloor"
  | "TrueStatementForward"
  | "OnlyOddEvenRangeElim"
  | "MostCommonElim"
  | "MostCommonForce"
  | "MostCommonCountCeil"
  | "ConsecIdentReverse"
  | "TrueStatementSelfRef"
  | "TrueStatementClaimInvalid"
  | "TrueStatementClaimValid"
  | "TrueStatementClaimKnownTrue"
  | "TrueStatementMatchForce"
  | "TrueStatementMatchElim"
  | "ConsecIdentForwardForce"
  | "ConsecIdentForwardElim"
  | "ConsecIdentForwardBothForce"
  | "EqualCountRangeElim"
  | "OnlySameOtherMatch"
  | "PrevSameNoneMatch"
  | "NextSameNoneMatch"
  | "OnlySameNoneMatch"
  | "OnlySameNoneForward"
  | "SameAsNegative"
  | "SameAsWhichForward"
  | "SameAsWhichReverse";

export type DeduceAction =
  | { type: "force"; qi: number; answer: Answer }
  | { type: "eliminate"; qi: number; oi: number }
  | { type: "eliminateMulti"; questionMask: number; optionMask: number };

export interface DeduceResult {
  action: DeduceAction;
  rule: DeduceRule;
}

/** One rendered hint step, produced by the Rust explain layer (via wasm) and
 *  rendered by `HintStep`: a single line, or a headed block of lines. */
export type ExplainStep =
  | { type: "simple"; text: string }
  | { type: "complex"; header: string; lines: string[] };

/** One solving step plus its rendered explanation — the unit the hint UI
 *  renders (`explain`) and the tutorial walks (applies `action`, narrates
 *  `explain`). The step is a deduction, or a lookahead elimination. */
export interface SolveStep {
  action: DeduceAction;
  explain: ExplainStep[];
}
