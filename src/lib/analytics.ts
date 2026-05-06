declare global {
  interface Navigator {
    standalone?: boolean;
  }
}

const ENDPOINT = "/analytics.php";

export function track(event: string, props?: Record<string, unknown>): void {
  const payload = JSON.stringify({ event, props });
  if (navigator.sendBeacon) {
    navigator.sendBeacon(ENDPOINT, new Blob([payload], { type: "application/json" }));
  } else {
    fetch(ENDPOINT, { method: "POST", body: payload, keepalive: true }).catch(() => {});
  }
}

function parseUA(): { os: string; browser: string } {
  const ua = navigator.userAgent;
  let os = "other";
  if (/iPad/.test(ua) || (/Mac/.test(ua) && navigator.maxTouchPoints > 1)) {
    os = "ipados";
  } else if (/iPhone|iPod/.test(ua)) {
    os = "ios";
  } else if (/Android/.test(ua)) {
    os = "android";
  } else if (/Win/.test(ua)) {
    os = "win";
  } else if (/Mac/.test(ua)) {
    os = "mac";
  } else if (/CrOS/.test(ua)) {
    os = "chromeos";
  } else if (/Linux/.test(ua)) {
    os = "linux";
  }

  let browser = "other";
  if (/Firefox\//.test(ua)) {
    browser = "firefox";
  } else if (/Edg\//.test(ua)) {
    browser = "edge";
  } else if (/Chrome\//.test(ua)) {
    browser = "chrome";
  } else if (/Safari\//.test(ua)) {
    browser = "safari";
  }

  return { os, browser };
}

export function getClientInfo(): { os: string; browser: string; standalone?: true } {
  const standalone =
    window.matchMedia("(display-mode: standalone)").matches ||
    navigator.standalone === true;
  return { ...parseUA(), standalone: standalone || undefined };
}

export function setupErrorTracking(): void {
  window.onerror = (msg, _src, _line, _col, error) => {
    track("js_error", {
      message: error?.message ?? (typeof msg === "string" ? msg : "unknown"),
      stack: error?.stack?.slice(0, 500),
      ua: navigator.userAgent,
      ...parseUA(),
    });
  };

  window.addEventListener("unhandledrejection", (e: PromiseRejectionEvent) => {
    const r: unknown = e.reason;
    track("js_error", {
      message: r instanceof Error ? r.message : String(r),
      stack: r instanceof Error ? r.stack?.slice(0, 500) : undefined,
      ua: navigator.userAgent,
      ...parseUA(),
    });
  });
}
