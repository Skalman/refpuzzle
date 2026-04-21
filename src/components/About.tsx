import { t } from "../i18n/index.ts";
import { Logo } from "./Logo.tsx";

export function About() {
  const s = t();
  return (
    <>
      <header class="app-header">
        <h1>
          <a href="/"><Logo />{s.app.title}</a>
        </h1>
      </header>
      <div class="about">
        <h2>{s.about.howToPlay}</h2>
        <ol>
          {s.about.howToPlaySteps.map((step) => (
            <li key={step}>{step}</li>
          ))}
        </ol>

        <h2>{s.about.howToSolve}</h2>
        <ol>
          {s.about.howToSolveSteps.map((step) => (
            <li key={step}>{step}</li>
          ))}
        </ol>

        <h2>{s.about.whatIs}</h2>
        <p>{s.about.description}</p>

        <p>
          <a href="/">Back to puzzles</a>
        </p>
      </div>
    </>
  );
}
