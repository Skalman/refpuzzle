import jsQR from "jsqr";

export default function startScanner(
  container: HTMLElement,
  onScan: (data: string) => void,
  onError: (msg: string) => void,
): () => void {
  const video = document.createElement("video");
  video.setAttribute("playsinline", "");
  video.className = "qr-scanner-video";
  container.appendChild(video);

  const canvas = document.createElement("canvas");
  canvas.style.display = "none";
  container.appendChild(canvas);
  const ctx = canvas.getContext("2d")!;

  let running = true;
  let stream: MediaStream | null = null;
  let raf = 0;

  navigator.mediaDevices
    .getUserMedia({ video: { facingMode: "environment" } })
    .then((s) => {
      stream = s;
      video.srcObject = s;
      video.play();
      scan();
    })
    .catch(() => onError("Camera access denied"));

  function scan() {
    if (!running) return;
    raf = requestAnimationFrame(scan);
    if (video.readyState < video.HAVE_ENOUGH_DATA) return;
    canvas.width = video.videoWidth;
    canvas.height = video.videoHeight;
    ctx.drawImage(video, 0, 0);
    const imageData = ctx.getImageData(0, 0, canvas.width, canvas.height);
    const result = jsQR(imageData.data, canvas.width, canvas.height);
    if (result?.data) {
      running = false;
      onScan(result.data);
    }
  }

  return () => {
    running = false;
    cancelAnimationFrame(raf);
    stream?.getTracks().forEach((t) => t.stop());
    container.textContent = "";
  };
}
