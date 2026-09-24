import { useRef, useState, useEffect } from "preact/hooks";
import { t } from "../i18n/index.ts";
import { startSync, pollSync, joinSync } from "../lib/sync.ts";
import { IconScan } from "./Icons.tsx";
import { Modal } from "./ui/Modal.tsx";
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
    <Modal title={s.sync.title} class="sync-dialog" onClose={onClose}>
      <p>{s.sync.description}</p>

      {!code && !scanning && (
        <>
          <Button variant="primary" class="sync-start-btn" onClick={handleStart} disabled={busy}>
            {s.sync.start}
          </Button>
          {error && <p class="sync-error">{error}</p>}

          <div class="sync-divider">
            <span>{s.sync.enterCode}</span>
          </div>

          <div class="sync-join">
            <input
              type="text"
              inputMode="numeric"
              pattern="[0-9]*"
              class="sync-code-input"
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

          <button class="sync-scan-btn" onClick={() => setScanning(true)}>
            <IconScan size="1em" /> {s.sync.scanQr}
          </button>
        </>
      )}

      {!code && scanning && (
        <>
          <div ref={scanBoxRef} class="qr-scanner" />
          <button class="sync-scan-btn" onClick={() => setScanning(false)}>
            {s.sync.enterCode}
          </button>
        </>
      )}

      {code && (
        <div class="sync-waiting">
          <QrImage value={syncUrl(code)} class="qr-image" />
          <div class="sync-code-display">{code}</div>
          <p class="sync-waiting-text">{s.sync.waiting}</p>
        </div>
      )}
    </Modal>
  );
}
