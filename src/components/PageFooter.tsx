import { useState } from "preact/hooks";
import type { ComponentChildren } from "preact";
import { Dialog } from "./ui/Dialog.tsx";
import { Link } from "./ui/Link.tsx";
import { DebugDialog } from "./DebugDialog.tsx";
import { contactAddress } from "../lib/contact.ts";
import { t } from "../i18n/index.ts";

function FooterLink({ onClick, children }: { onClick: () => void; children: ComponentChildren }) {
  return (
    <button
      class="cursor-pointer border-none bg-transparent px-2 py-1 text-muted [font:inherit] hover:text-default hover:underline"
      onClick={onClick}
    >
      {children}
    </button>
  );
}

function FooterSeparator() {
  return (
    <span class="text-muted" aria-hidden="true">
      ·
    </span>
  );
}

export function PageFooter() {
  const s = t();
  const [openNote, setOpenNote] = useState<"privacy" | "contact" | "debug" | null>(null);
  const contact = contactAddress();
  const close = () => setOpenNote(null);
  // Centered by flex rather than text-align, so the dialog it hosts keeps its text left-aligned.
  return (
    <footer class="mt-8 flex items-center justify-center text-chrome">
      <FooterLink onClick={() => setOpenNote("privacy")}>{s.privacy.link}</FooterLink>
      {/* No address configured for this build: nothing to offer. */}
      {contact && (
        <>
          <FooterSeparator />
          <FooterLink onClick={() => setOpenNote("contact")}>{s.contact.link}</FooterLink>
        </>
      )}
      {import.meta.env.DEV && (
        <>
          <FooterSeparator />
          <FooterLink onClick={() => setOpenNote("debug")}>Debug</FooterLink>
          {openNote === "debug" && <DebugDialog onClose={close} />}
        </>
      )}
      {(openNote === "privacy" || openNote === "contact") && (
        <Dialog title={s[openNote].title} onClose={close}>
          {openNote === "privacy" ? (
            <>
              {s.privacy.paragraphs.map((x) => (
                <p key={x} class="mb-2">
                  {x}
                </p>
              ))}
              {contact && (
                <p class="mb-2">
                  {s.privacy.contactPrompt} <Link href={`mailto:${contact}`}>{contact}</Link>
                </p>
              )}
            </>
          ) : (
            <>
              <p class="mb-2">{s.contact.body}</p>
              <p class="mb-2">
                <Link href={`mailto:${contact}`}>{contact}</Link>
              </p>
            </>
          )}
        </Dialog>
      )}
    </footer>
  );
}
