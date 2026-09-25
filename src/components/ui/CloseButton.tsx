import { t } from "../../i18n/index.ts";

/** The × that dismisses a panel. */
export function CloseButton({ onClick }: { onClick: () => void }) {
  return (
    <button
      class="cursor-pointer border-none bg-transparent p-1 text-title leading-none text-muted hover:text-default"
      onClick={onClick}
      aria-label={t().aria.close}
    >
      &times;
    </button>
  );
}
