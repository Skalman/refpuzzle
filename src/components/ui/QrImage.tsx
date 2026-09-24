import { useEffect, useRef } from "preact/hooks";

/** A QR code for `value`, drawn once the encoder's chunk has loaded. */
export function QrImage({ value, class: className }: { value: string; class: string }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    let canceled = false;
    void import("../QrCode.tsx").then(({ default: renderQrSvg }) => {
      if (!canceled && ref.current) ref.current.innerHTML = renderQrSvg(value);
    });
    return () => {
      canceled = true;
    };
  }, [value]);
  return <div ref={ref} class={className} />;
}
