//! Diagnostic renderings of a `QuestionType` — the compact tag `check`/`reference`
//! print, and the JSON object the generator's `--trace` emits. Distinct from
//! `render`, which is the player-facing prose.

use crate::types::*;

/// Debug tag for a question type, e.g. `CountAnswerBefore(A,q=3)`.
pub fn format_type_tag(qt: &QuestionType) -> String {
    match qt {
        QuestionType::CountAnswer { answer } => format!("CountAnswer({})", answer.as_char()),
        QuestionType::CountAnswerBefore {
            answer,
            before_index,
        } => format!("CountAnswerBefore({},q={})", answer.as_char(), before_index),
        QuestionType::CountAnswerAfter {
            answer,
            after_index,
        } => format!("CountAnswerAfter({},q={})", answer.as_char(), after_index),
        QuestionType::ClosestAfter {
            answer,
            after_index,
        } => format!("ClosestAfter({},q={})", answer.as_char(), after_index),
        QuestionType::ClosestBefore {
            answer,
            before_index,
        } => format!("ClosestBefore({},q={})", answer.as_char(), before_index),
        QuestionType::FirstWith { answer } => format!("FirstWith({})", answer.as_char()),
        QuestionType::LastWith { answer } => format!("LastWith({})", answer.as_char()),
        QuestionType::OnlyOdd { answer } => format!("OnlyOdd({})", answer.as_char()),
        QuestionType::OnlyEven { answer } => format!("OnlyEven({})", answer.as_char()),
        QuestionType::EqualCount { answer } => format!("EqualCount({})", answer.as_char()),
        QuestionType::AnswerOf { question_index } => format!("AnswerOf(q={})", question_index),
        QuestionType::LetterDist { question_index } => format!("LetterDist(q={})", question_index),
        QuestionType::SameAsWhich { question_index } => {
            format!("SameAsWhich(q={})", question_index)
        }
        _ => format!("{:?}", qt),
    }
}

/// A TrueStmt claim's question type as a `{ type, ...params }` JSON object, for the
/// generator's per-question trace line.
pub(crate) fn format_stmt_qt(qt: &QuestionType) -> serde_json::Value {
    let type_name = match qt {
        QuestionType::CountAnswer { .. } => "CountAnswer",
        QuestionType::CountConsonant => "CountConsonant",
        QuestionType::CountVowel => "CountVowel",
        QuestionType::CountAnswerAfter { .. } => "CountAnswerAfter",
        QuestionType::CountAnswerBefore { .. } => "CountAnswerBefore",
        QuestionType::AnswerOf { .. } => "AnswerOf",
        QuestionType::FirstWith { .. } => "FirstWith",
        QuestionType::LastWith { .. } => "LastWith",
        QuestionType::MostCommon => "MostCommon",
        QuestionType::LeastCommon => "LeastCommon",
        QuestionType::MostCommonCount => "MostCommonCount",
        QuestionType::NoOtherHasAnswer => "NoOtherHasAnswer",
        QuestionType::ConsecIdent => "ConsecIdent",
        QuestionType::OnlyOdd { .. } => "OnlyOdd",
        QuestionType::OnlyEven { .. } => "OnlyEven",
        QuestionType::EqualCount { .. } => "EqualCount",
        QuestionType::ClosestAfter { .. } => "ClosestAfter",
        QuestionType::ClosestBefore { .. } => "ClosestBefore",
        // Never statement subjects (`check_form::check_stmt_kind` rejects them).
        QuestionType::PrevSame
        | QuestionType::NextSame
        | QuestionType::OnlySame
        | QuestionType::SameAs
        | QuestionType::SameAsWhich { .. }
        | QuestionType::AnswerIsSelf
        | QuestionType::LetterDist { .. }
        | QuestionType::TrueStmt => "Invalid",
    };
    let mut obj = serde_json::Map::new();
    obj.insert("type".into(), serde_json::json!(type_name));
    match *qt {
        QuestionType::CountAnswer { answer }
        | QuestionType::FirstWith { answer }
        | QuestionType::LastWith { answer }
        | QuestionType::OnlyOdd { answer }
        | QuestionType::OnlyEven { answer }
        | QuestionType::EqualCount { answer } => {
            obj.insert(
                "answer".into(),
                serde_json::json!(answer.as_char().to_string()),
            );
        }
        QuestionType::CountAnswerAfter {
            answer,
            after_index,
        } => {
            obj.insert(
                "answer".into(),
                serde_json::json!(answer.as_char().to_string()),
            );
            obj.insert("afterIndex".into(), serde_json::json!(after_index));
        }
        QuestionType::CountAnswerBefore {
            answer,
            before_index,
        } => {
            obj.insert(
                "answer".into(),
                serde_json::json!(answer.as_char().to_string()),
            );
            obj.insert("beforeIndex".into(), serde_json::json!(before_index));
        }
        QuestionType::ClosestAfter {
            answer,
            after_index,
        } => {
            obj.insert(
                "answer".into(),
                serde_json::json!(answer.as_char().to_string()),
            );
            obj.insert("afterIndex".into(), serde_json::json!(after_index));
        }
        QuestionType::ClosestBefore {
            answer,
            before_index,
        } => {
            obj.insert(
                "answer".into(),
                serde_json::json!(answer.as_char().to_string()),
            );
            obj.insert("beforeIndex".into(), serde_json::json!(before_index));
        }
        QuestionType::AnswerOf { question_index }
        | QuestionType::LetterDist { question_index }
        | QuestionType::SameAsWhich { question_index } => {
            obj.insert("questionIndex".into(), serde_json::json!(question_index));
        }
        _ => {}
    }
    serde_json::Value::Object(obj)
}
