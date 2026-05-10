import { IconX } from "./Icons.tsx";

interface Props {
  onStart: () => void;
  onDismiss: () => void;
}

export function TutorialWelcome({ onStart, onDismiss }: Props) {
  return (
    <div class="tutorial-overlay" onClick={onDismiss}>
      <div class="tutorial-bubble tutorial-welcome-bubble" onClick={(e) => e.stopPropagation()}>
        <button class="tutorial-skip" onClick={onDismiss} aria-label="Dismiss">
          <IconX size="1.2em" />
        </button>
        <div class="tutorial-welcome-title">Welcome to Refpuzzle!</div>
        <div class="tutorial-explain">
          A self-referential logic puzzle. Let's walk through your first one together.
        </div>
        <button class="tutorial-start-btn" onClick={onStart}>
          Start tutorial
        </button>
      </div>
    </div>
  );
}
