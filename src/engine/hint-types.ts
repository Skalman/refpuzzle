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

export interface LookaheadResult {
  /**
   * The disproven hypothesis — "suppose question `assumptionQi` were
   * `assumptionAnswer`". Because that assumption leads to a contradiction, this
   * is also the option the hint eliminates: the assumed option and the
   * eliminated option are always the same in this lookahead.
   */
  assumptionQi: number;
  assumptionAnswer: Answer;
  /** The deduction steps from the assumption to the contradiction. */
  chain: DeduceResult[];
  /** The question at which the contradiction surfaced (may differ from `assumptionQi`). */
  contradictionQi: number;
}
