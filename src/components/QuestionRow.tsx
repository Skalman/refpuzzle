import type { QuestionDef } from "../engine/types.ts";
import { OptionButton } from "./OptionButton.tsx";

type OptionMark = "unmarked" | "incorrect" | "correct";

interface Props {
	index: number;
	question: QuestionDef;
	marks: [OptionMark, OptionMark, OptionMark, OptionMark, OptionMark];
	validity: "neutral" | "valid" | "invalid";
	onOptionClick: (optionIndex: number) => void;
}

const LONG_THRESHOLD = 12;

export function QuestionRow({ index, question, marks, validity, onOptionClick }: Props) {
	const isLong = question.options.some((o) => o.label.length > LONG_THRESHOLD);

	return (
		<div class="question-row">
			<div class={`validity-bar ${validity}`} />
			<div class="question-body">
				<div class="question-header">
					<span class="question-num">{index + 1}.</span>
					<span class="question-text">{question.text}</span>
				</div>
				<div class={`question-options ${isLong ? "options-vertical" : ""}`}>
					{question.options.map((opt, oi) => (
						<OptionButton
							key={opt.label}
							index={oi}
							label={opt.label}
							mark={marks[oi]}
							onClick={() => onOptionClick(oi)}
						/>
					))}
				</div>
			</div>
		</div>
	);
}
