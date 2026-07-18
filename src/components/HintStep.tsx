import type { ExplainStep } from "../engine/hint-types.ts";
import { t } from "../i18n/index.ts";

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
  if (step.type === "look") return <>{t().hint.tryLooking(step.qis)}</>;
  return <>{step.text}</>;
}
