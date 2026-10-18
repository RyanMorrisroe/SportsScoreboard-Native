import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const appVersion = process.env.APP_VERSION;
if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)-wrapper\.(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(appVersion ?? "")) {
  throw new Error(`Invalid composite app version: ${appVersion ?? "<empty>"}`);
}
const androidVersionCode = Number(process.env.ANDROID_VERSION_CODE);
if (!Number.isSafeInteger(androidVersionCode) || androidVersionCode < 1 || androidVersionCode > 2_100_000_000) {
  throw new Error(`Invalid Android version code: ${process.env.ANDROID_VERSION_CODE ?? "<empty>"}`);
}

const root = fileURLToPath(new URL("../", import.meta.url));
const cargoPath = resolve(root, "src-tauri/Cargo.toml");
const cargoLines = readFileSync(cargoPath, "utf8").split(/\r?\n/);
let inPackageSection = false;
let updatedVersion = false;

for (let index = 0; index < cargoLines.length; index += 1) {
  const section = /^\s*\[([^\]]+)\]\s*$/.exec(cargoLines[index]);
  if (section) {
    inPackageSection = section[1] === "package";
  } else if (inPackageSection && /^version\s*=/.test(cargoLines[index])) {
    cargoLines[index] = `version = "${appVersion}"`;
    updatedVersion = true;
    break;
  }
}

if (!updatedVersion) {
  throw new Error("Could not find the [package] version in src-tauri/Cargo.toml");
}

writeFileSync(cargoPath, cargoLines.join("\n"));
writeFileSync(
  resolve(root, "src-tauri/tauri.release.conf.json"),
  `${JSON.stringify({
    version: appVersion,
    bundle: { android: { versionCode: androidVersionCode } },
  }, null, 2)}\n`,
);