import { Fragment } from "preact";
import { t } from "../i18n/index.ts";
import { CloseButton } from "./ui/CloseButton.tsx";

const IS_MAC = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.userAgent);
const MOD = IS_MAC ? "⌘" : "Ctrl";

function shortcutGroups(s: ReturnType<typeof t>) {
  return [
    {
      title: s.keyboard.navigation,
      shortcuts: [
        { keys: ["1", "2", "…", "12"], desc: s.keyboard.jumpToQuestion },
        { keys: ["↑", "K"], desc: s.keyboard.prevQuestion },
        { keys: ["↓", "J"], desc: s.keyboard.nextQuestion },
        { keys: ["←", "→"], desc: s.keyboard.moveOptions },
        { keys: ["A", "B", "…", "E"], desc: s.keyboard.selectOption },
        { keys: ["Enter", "Space"], desc: s.keyboard.toggleOption },
        { keys: ["[", "]"], desc: s.keyboard.prevNextDifficulty },
      ],
    },
    {
      title: s.keyboard.actions,
      shortcuts: [
        { keys: [`${MOD}+Z`], desc: s.keyboard.undo },
        { keys: [`${MOD}+Shift+Z`], desc: s.keyboard.redo },
        { keys: ["H"], desc: s.keyboard.hint },
        { keys: ["P"], desc: s.keyboard.checkpoint },
        { keys: ["Escape"], desc: s.keyboard.closeCancel },
      ],
    },
    {
      title: s.keyboard.general,
      shortcuts: [
        { keys: ["?"], desc: s.keyboard.toggleHelp },
        { keys: ["Tab", "Shift+Tab"], desc: s.keyboard.navigateSections },
      ],
    },
  ];
}

export function KeyboardShortcutList() {
  const s = t();
  const groups = shortcutGroups(s);
  return (
    <div>
      {groups.map((group) => (
        <div key={group.title}>
          <h4 class="mb-1 text-body font-bold text-default">{group.title}</h4>
          <dl class="m-0">
            {group.shortcuts.map((sc) => (
              <div key={sc.keys[0]} class="flex items-baseline gap-3 py-[0.15rem] text-chrome">
                <dt class="w-36 shrink-0 text-right">
                  {sc.keys.map((k, i) => (
                    <Fragment key={k}>
                      {i > 0 && " / "}
                      <kbd class="rounded-sm border bg-hover px-[0.35rem] py-[0.05rem] text-chrome font-semibold font-[inherit]">
                        {k}
                      </kbd>
                    </Fragment>
                  ))}
                </dt>
                <dd class="m-0 text-muted">{sc.desc}</dd>
              </div>
            ))}
          </dl>
        </div>
      ))}
    </div>
  );
}

export function KeyboardHelp({ onClose }: { onClose: () => void }) {
  const s = t();
  return (
    <div
      class="fixed inset-0 z-1000 flex items-center justify-center bg-backdrop p-safe-4"
      onClick={onClose}
    >
      <div
        // 100% is the backdrop's content box, which already excludes the safe areas.
        class="max-h-[min(80vh,100%)] w-9/10 max-w-96 overflow-y-auto rounded-xl border bg-surface p-5 text-default shadow-dialog"
        onClick={(e) => e.stopPropagation()}
      >
        <div class="mb-3 flex items-center justify-between">
          <strong class="text-dialog">{s.keyboard.title}</strong>
          <CloseButton onClick={onClose} />
        </div>
        <KeyboardShortcutList />
      </div>
    </div>
  );
}
