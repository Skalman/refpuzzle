import { t } from "../../i18n/index.ts";

/** The × that dismisses a panel. */
export function CloseButton({ onClick }: { onClick: () => void }) {
  return (
    <button class="help-close" onClick={onClick} aria-label={t().aria.close}>
      &times;
    </button>
  );
}
