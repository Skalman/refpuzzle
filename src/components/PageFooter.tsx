import { useState } from "preact/hooks";
import type { ComponentChildren } from "preact";
import { Modal } from "./ui/Modal.tsx";
import { DebugDialog } from "./DebugDialog.tsx";
import { contactAddress } from "../lib/contact.ts";
import { t } from "../i18n/index.ts";

function FooterLink({ onClick, children }: { onClick: () => void; children: ComponentChildren }) {
  return (
    <button class="footer-link" onClick={onClick}>
      {children}
    </button>
  );
}

function FooterSeparator() {
  return (
    <span class="footer-separator" aria-hidden="true">
      ·
    </span>
  );
}

export function PageFooter() {
  const s = t();
  const [openNote, setOpenNote] = useState<"privacy" | "contact" | "debug" | null>(null);
  const contact = contactAddress();
  const close = () => setOpenNote(null);
  return (
    <footer class="page-footer">
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
        <Modal title={s[openNote].title} onClose={close}>
          {openNote === "privacy" ? (
            <>
              {s.privacy.paragraphs.map((x) => (
                <p key={x}>{x}</p>
              ))}
              {contact && (
                <p>
                  {s.privacy.contactPrompt} <a href={`mailto:${contact}`}>{contact}</a>
                </p>
              )}
            </>
          ) : (
            <>
              <p>{s.contact.body}</p>
              <p>
                <a href={`mailto:${contact}`}>{contact}</a>
              </p>
            </>
          )}
        </Modal>
      )}
    </footer>
  );
}
