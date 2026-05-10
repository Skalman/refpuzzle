import type { ExplainStep } from "../engine/explain.ts";

export function HintStep({ step }: { step: ExplainStep }) {
  if (step.type === "complex") {
    return (
      <div>
        {step.header}
        <ul class="hint-list">
          {step.lines.map((line, i) => (
            // oxlint-disable-next-line react/no-array-index-key
            <li key={i}>{line}</li>
          ))}
        </ul>
      </div>
    );
  }
  return <>{step.text}</>;
}
