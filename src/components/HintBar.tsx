import { t } from "../i18n/index.ts";

interface Props {
	contradictionsOn: boolean;
	onToggleContradictions: () => void;
}

export function HintBar({ contradictionsOn, onToggleContradictions }: Props) {
	const s = t();
	return (
		<div class="puzzle-actions">
			<button
				onClick={onToggleContradictions}
				style={contradictionsOn ? "background: var(--accent-soft); color: var(--accent); border-color: var(--accent);" : ""}
			>
				{s.hints.contradictions}: {contradictionsOn ? "ON" : "OFF"}
			</button>
		</div>
	);
}
