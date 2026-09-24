import { useState, useEffect } from "preact/hooks";
import type { ComponentChildren } from "preact";
import { IconCheck, IconX, IconPin } from "./Icons.tsx";
import type { HelpIcon } from "../i18n/en.ts";
import { classNames } from "../lib/classNames.ts";
import { pointerKind } from "../lib/pointer.ts";
import { t } from "../i18n/index.ts";

const HELP_ICONS: Record<HelpIcon, ComponentChildren> = {
  incorrect: <IconX size="0.9em" strokeWidth={3} class="icon-incorrect" />,
  correct: <IconCheck size="0.9em" strokeWidth={3} class="icon-correct" />,
  checkpoint: <IconPin size="0.9em" class="icon-checkpoint" />,
};

export function InlineHelp({ highlight }: { highlight?: boolean }) {
  const s = t();
  const [firstVisit, setFirstVisit] = useState(() => {
    try {
      return !localStorage.getItem("refpuzzle:onboarded");
    } catch {
      return false;
    }
  });

  useEffect(() => {
    if (!firstVisit) return undefined;
    try {
      localStorage.setItem("refpuzzle:onboarded", "1");
    } catch {
      // ignore
    }
    const timer = setTimeout(() => setFirstVisit(false), 15000);
    return () => clearTimeout(timer);
  }, [firstVisit]);

  const show = highlight || firstVisit;

  return (
    <div class="inline-help">
      <div class={classNames("how-to-play", show && "how-to-play--first-visit")}>
        <h4>{s.help.title}</h4>
        <p class="how-to-goal">{s.help.goal}</p>
        <ol>
          {s.help.howToPlaySteps(pointerKind()).map((step) => (
            <li key={step.text}>
              {step.text}
              {step.icon && (
                <>
                  {" "}
                  <span class="nowrap">({HELP_ICONS[step.icon]})</span>
                </>
              )}
            </li>
          ))}
        </ol>
      </div>
      <h4>{s.help.whatIs}</h4>
      {s.help.descriptionParagraphs.map((p) => (
        <p key={p}>{p}</p>
      ))}
    </div>
  );
}
