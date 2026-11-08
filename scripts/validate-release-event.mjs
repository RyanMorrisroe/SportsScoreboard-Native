import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

export function validateSpaReleaseEvent({ tag, version, sha }) {
  const match = /^v((0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*))$/.exec(
    tag ?? "",
  );
  if (!match) {
    throw new Error(`Unsupported SPA release tag: ${tag ?? ""}`);
  }
  if (match[1] !== version) {
    throw new Error("SPA tag and payload version do not match");
  }
  if (!/^[0-9a-f]{40}$/.test(sha ?? "")) {
    throw new Error("Invalid SPA commit SHA");
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    validateSpaReleaseEvent({
      tag: process.env.SPA_TAG,
      version: process.env.SPA_VERSION,
      sha: process.env.SPA_SHA,
    });
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}