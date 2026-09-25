import { useRef, useState, useEffect } from "preact/hooks";
import { t } from "../i18n/index.ts";
import { startSync, pollSync, joinSync } from "../lib/sync.ts";
import { IconScan } from "./Icons.tsx";
import { Dialog } from "./ui/Dialog.tsx";
import { Button } from "./ui/Button.tsx";
import { QrImage } from "./ui/QrImage.tsx";

/** The link a scanner follows to join this sync session. */
function syncUrl(code: string): string {
  return `${window.location.origin}/sync#${code}`;
}

export function SyncDialog({
  onImport,
  onClose,
}: {
  onImport: (json: string) => void;
  onClose: () => void;
}) {
  const s = t();
  const [code, setCode] = useState<string | null>(null);
  const [inputCode, setInputCode] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [scanning, setScanning] = useState(false);
  const scanBoxRef = useRef<HTMLDivElement>(null);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(
    () => () => {
      if (pollRef.current) clearInterval(pollRef.current);
    },
    [],
  );

  function handleStart() {
    setBusy(true);
    setError(null);
    startSync()
      .then((c) => {
        setCode(c);
        setBusy(false);
        pollRef.current = setInterval(() => {
          pollSync(c).then((json) => {
            if (json) {
              if (pollRef.current) clearInterval(pollRef.current);
              pollRef.current = null;
              onImport(json);
            }
          });
        }, 2000);
      })
      .catch(() => {
        setError(s.sync.error);
        setBusy(false);
      });
  }

  function handleJoinCode(c: string) {
    setBusy(true);
    setError(null);
    setScanning(false);
    joinSync(c)
      .then((json) => {
        onImport(json);
      })
      .catch(() => {
        setError(s.sync.expired);
        setBusy(false);
      });
  }

  function handleScan(data: string) {
    const match = data.match(/\/sync#(\d{6})$/);
    if (match) {
      handleJoinCode(match[1]);
    } else if (/^\d{6}$/.test(data)) {
      handleJoinCode(data);
    } else {
      setError(s.sync.expired);
      setScanning(false);
    }
  }

  // The camera loop outlives the render that armed it, so it reads the handler
  // through a ref; rebinding it would restart the camera on every render.
  const onScanRef = useRef(handleScan);
  onScanRef.current = handleScan;

  useEffect(() => {
    const box = scanBoxRef.current;
    if (!scanning || !box) return undefined;
    let stop: (() => void) | null = null;
    let canceled = false;
    void import("./QrScanner.tsx").then(({ default: startScan }) => {
      if (canceled) return;
      stop = startScan(
        box,
        (data) => onScanRef.current(data),
        (msg) => {
          setError(msg);
          setScanning(false);
        },
      );
    });
    return () => {
      canceled = true;
      stop?.();
    };
  }, [scanning]);

  return (
    <Dialog title={s.sync.title} widthClass="max-w-88" onClose={onClose}>
      <p class="mb-2">{s.sync.description}</p>

      {!code && !scanning && (
        <>
          <Button variant="primary" size="lg" class="w-full" onClick={handleStart} disabled={busy}>
            {s.sync.start}
          </Button>
          {error && (
            <p class="my-2 rounded-md bg-invalid-soft px-3 py-2 text-center text-body text-invalid">
              {error}
            </p>
          )}

          <div class="my-4 flex items-center gap-3 text-chrome text-muted before:flex-1 before:border-t after:flex-1 after:border-t">
            <span>{s.sync.enterCode}</span>
          </div>

          <div class="flex gap-2">
            <input
              type="text"
              inputMode="numeric"
              pattern="[0-9]*"
              class="flex-1 rounded-md border bg-surface px-2.5 py-1.5 text-center text-section tracking-[0.15em] text-default"
              maxLength={6}
              placeholder={s.sync.codePlaceholder}
              value={inputCode}
              onInput={(e) => {
                const el = e.target;
                if (el instanceof HTMLInputElement) {
                  setInputCode(el.value);
                  if (el.value.trim().length === 6) handleJoinCode(el.value.trim());
                }
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter" && inputCode.trim().length === 6)
                  handleJoinCode(inputCode.trim());
              }}
            />
            <Button
              variant="primary"
              onClick={() => handleJoinCode(inputCode.trim())}
              disabled={busy || inputCode.trim().length !== 6}
            >
              {s.sync.join}
            </Button>
          </div>

          <Button
            variant="outline-muted"
            class="mt-1 w-full"
            icon={<IconScan />}
            onClick={() => setScanning(true)}
          >
            {s.sync.scanQr}
          </Button>
        </>
      )}

      {!code && scanning && (
        <>
          <div ref={scanBoxRef} class="mb-2 w-full overflow-hidden rounded-lg" />
          <Button variant="outline-muted" class="mt-1 w-full" onClick={() => setScanning(false)}>
            {s.sync.enterCode}
          </Button>
        </>
      )}

      {code && (
        <div class="py-2 text-center">
          <QrImage value={syncUrl(code)} class="mx-auto size-30 rounded-sm bg-qr-bg" />
          <div class="py-3 text-display font-bold tracking-[0.25em] tabular-nums">{code}</div>
          <p class="mb-2 text-body text-muted">{s.sync.waiting}</p>
        </div>
      )}
    </Dialog>
  );
}
