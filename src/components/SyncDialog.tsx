import { useRef, useState, useEffect } from "preact/hooks";
import { t } from "../i18n/index.ts";
import { startSync, pollSync, joinSync } from "../lib/sync.ts";

export function SyncDialog({
  onImport,
  onClose,
}: {
  onImport: (json: string) => void;
  onClose: () => void;
}) {
  const s = t();
  const ref = useRef<HTMLDialogElement>(null);
  const [code, setCode] = useState<string | null>(null);
  const [inputCode, setInputCode] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [scanning, setScanning] = useState(false);
  const stopScanRef = useRef<(() => void) | null>(null);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    ref.current?.showModal();
    return () => {
      if (pollRef.current) clearInterval(pollRef.current);
      stopScanRef.current?.();
    };
  }, []);

  function syncUrl(c: string) {
    return `${window.location.origin}/sync#${c}`;
  }

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

  return (
    <dialog
      ref={ref}
      class="help-panel sync-dialog"
      onClose={onClose}
      onClick={(e) => {
        if (e.target === ref.current) onClose();
      }}
    >
      <div class="help-panel-inner">
        <div class="help-panel-header">
          <h3>{s.sync.title}</h3>
          <button class="help-close" onClick={onClose} aria-label={s.aria.close}>
            &times;
          </button>
        </div>
        <p>{s.sync.description}</p>

        {!code && !scanning && (
          <>
            <button class="primary-btn sync-start-btn" onClick={handleStart} disabled={busy}>
              {s.sync.start}
            </button>
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
              <button
                class="primary-btn"
                onClick={() => handleJoinCode(inputCode.trim())}
                disabled={busy || inputCode.trim().length !== 6}
              >
                {s.sync.join}
              </button>
            </div>

            <button class="sync-scan-btn" onClick={() => setScanning(true)}>
              {s.sync.scanQr}
            </button>
          </>
        )}

        {!code && scanning && (
          <>
            <div
              class="qr-scanner"
              ref={(el) => {
                if (!el) return;
                import("./QrScanner.tsx").then(({ default: startScan }) => {
                  stopScanRef.current?.();
                  stopScanRef.current = startScan(el, handleScan, (msg) => {
                    setError(msg);
                    setScanning(false);
                  });
                });
              }}
            />
            <button
              class="sync-scan-btn"
              onClick={() => {
                stopScanRef.current?.();
                stopScanRef.current = null;
                setScanning(false);
              }}
            >
              {s.sync.enterCode}
            </button>
          </>
        )}

        {code && (
          <div class="sync-waiting">
            <div
              class="qr-image"
              ref={(el) => {
                if (!el) return;
                import("./QrCode.tsx").then(({ default: renderQrSvg }) => {
                  el.innerHTML = renderQrSvg(syncUrl(code));
                });
              }}
            />
            <div class="sync-code-display">{code}</div>
            <p class="sync-waiting-text">{s.sync.waiting}</p>
          </div>
        )}
      </div>
    </dialog>
  );
}
