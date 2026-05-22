use crate::types::*;

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
