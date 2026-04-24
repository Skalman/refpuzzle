import { t } from "../i18n/index.ts";
import { allPuzzles } from "../puzzles/index.ts";
import { hasState } from "../lib/store.ts";
import { Logo } from "./Logo.tsx";

const stars = (n: number) => "\u2605".repeat(n) + "\u2606".repeat(5 - n);

export function PuzzleList() {
  const s = t();
  return (
    <>
      <header class="app-header">
        <h1>
          <Logo />
          {s.app.title}
        </h1>
        <div class="header-actions">
          <a href="/about">About</a>
          <ThemeToggle />
        </div>
      </header>
      <p style={{ color: "var(--text-muted)", marginBottom: "1rem" }}>{s.puzzleList.subtitle}</p>
      <div class="puzzle-grid">
        {allPuzzles.map((p) => {
          const state = hasState(p.id);
          return (
            <a key={p.id} href={`/puzzle/${p.id}`} class="puzzle-card">
              <div class="title">{p.title}</div>
              <div class="meta">
                <span class="difficulty">{stars(p.difficulty)}</span> {p.questions.length}{" "}
                {s.puzzleList.questions}
              </div>
              {state.completed && <div class="status">{s.puzzleList.solved}</div>}
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
      localStorage.setItem("refpuzzle:theme", "light");
    } else if (current === "light") {
      html.removeAttribute("data-theme");
      localStorage.removeItem("refpuzzle:theme");
    } else {
      html.setAttribute("data-theme", "dark");
      localStorage.setItem("refpuzzle:theme", "dark");
    }
  }

  return (
    <button class="theme-toggle" onClick={cycle} aria-label="Toggle theme">
      theme
    </button>
  );
}
