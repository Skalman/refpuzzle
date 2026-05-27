import { execSync } from "node:child_process";

export function x(command: string, options?: { input?: string }) {
  try {
    return execSync(command, {
      encoding: "utf-8",
      maxBuffer: 50 * 1024 * 1024,
      input: options?.input,
      stdio: ["pipe", "pipe", "pipe"],
    });
  } catch (err) {
    if (err && typeof err === "object" && "stderr" in err) {
      const stderr = (err as { stderr: unknown }).stderr;
      if (typeof stderr === "string" && stderr) process.stderr.write(stderr);
    }
    throw err;
  }
}
