import { useState, useEffect } from "preact/hooks";
import type { ComponentChildren } from "preact";
import { IconCheck, IconX, IconPin } from "./Icons.tsx";
import type { HelpIcon } from "../i18n/en.ts";
import { classNames } from "../lib/classNames.ts";
import { pointerKind } from "../lib/pointer.ts";
import { t } from "../i18n/index.ts";

const HELP_ICONS: Record<HelpIcon, ComponentChildren> = {
  incorrect: <IconX size="0.9em" strokeWidth={3} class="text-invalid" />,
  correct: <IconCheck size="0.9em" strokeWidth={3} class="text-valid" />,
  checkpoint: <IconPin size="0.9em" class="text-valid" />,
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
    <div class="mx-auto mt-8 max-w-150 p-4 text-chrome leading-normal text-muted">
      <div
        class={classNames(
          "-mx-3 rounded-lg border border-solid px-3 py-2 transition-[background,border-color] duration-2000",
          show ? "border-accent bg-accent-soft" : "border-transparent",
        )}
      >
        <h4 class="mb-[0.3rem] text-body font-bold text-default">{s.help.title}</h4>
        <p class="mb-2 font-medium text-default">{s.help.goal}</p>
        <ol class="m-0 list-decimal pl-6">
          {s.help.howToPlaySteps(pointerKind()).map((step) => (
            <li key={step.text}>
              {step.text}
              {step.icon && (
                <>
                  {" "}
                  <span class="whitespace-nowrap [&_svg]:align-[-0.125em]">
                    ({HELP_ICONS[step.icon]})
                  </span>
                </>
              )}
            </li>
          ))}
        </ol>
      </div>
      <h4 class="mt-[0.8rem] mb-[0.3rem] text-body font-bold text-default">{s.help.whatIs}</h4>
      {s.help.descriptionParagraphs.map((p) => (
        <p key={p} class="mb-2">
          {p}
        </p>
      ))}
    </div>
  );
}
