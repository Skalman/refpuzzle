import { t } from "../i18n/index.ts";
import { allPuzzles } from "../puzzles/index.ts";
import { loadState } from "../lib/store.ts";

const stars = (n: number) => "\u2605".repeat(n) + "\u2606".repeat(5 - n);

export function PuzzleList() {
	const s = t();
	return (
		<>
			<header class="app-header">
				<h1>{s.app.title}</h1>
				<div class="header-actions">
					<a href="/about">{s.about.title}</a>
					<ThemeToggle />
				</div>
			</header>
			<p style={{ color: "var(--text-muted)", marginBottom: "1rem" }}>
				{s.puzzleList.subtitle}
			</p>
			<div class="puzzle-grid">
				{allPuzzles.map((p) => {
					const state = loadState(p.id);
					return (
						<a key={p.id} href={`/puzzle/${p.id}`} class="puzzle-card">
							<div class="title">{p.title}</div>
							<div class="meta">
								<span class="difficulty">{stars(p.difficulty)}</span>
								{" "}
								{p.questions.length} {s.puzzleList.questions}
							</div>
							{state?.completed && (
								<div class="status">{s.puzzleList.solved}</div>
							)}
						</a>
					);
				})}
			</div>
		</>
	);
}

function ThemeToggle() {
	function cycle() {
		const html = document.documentElement;
		const current = html.getAttribute("data-theme");
		if (current === "dark") {
			html.setAttribute("data-theme", "light");
		} else if (current === "light") {
			html.removeAttribute("data-theme");
		} else {
			html.setAttribute("data-theme", "dark");
		}
	}

	return (
		<button class="theme-toggle" onClick={cycle} aria-label="Toggle theme">
			theme
		</button>
	);
}
