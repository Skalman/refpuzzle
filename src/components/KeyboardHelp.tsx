import { t } from "../i18n/index.ts";

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
    <div class="keyboard-shortcut-list">
      {groups.map((group) => (
        <div key={group.title}>
          <h4>{group.title}</h4>
          <dl class="shortcut-dl">
            {group.shortcuts.map((sc) => (
              <div key={sc.keys[0]} class="shortcut-row">
                <dt>
                  {sc.keys.map((k, i) => (
                    <>
                      {i > 0 && " / "}
                      <kbd>{k}</kbd>
                    </>
                  ))}
                </dt>
                <dd>{sc.desc}</dd>
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
    <div class="keyboard-help" onClick={onClose}>
      <div class="keyboard-help-inner" onClick={(e) => e.stopPropagation()}>
        <div class="keyboard-help-header">
          <strong>{s.keyboard.title}</strong>
          <button class="help-close" onClick={onClose} aria-label={s.aria.close}>
            &times;
          </button>
        </div>
        <KeyboardShortcutList />
      </div>
    </div>
  );
}
