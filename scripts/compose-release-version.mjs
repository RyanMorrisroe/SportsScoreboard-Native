import { appendFileSync, readFileSync } from "node:fs";

const releaseTag = process.env.SPA_TAG;
const match = /^v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.exec(releaseTag ?? "");
if (!match) {
  throw new Error(`Expected a stable SPA release tag like v1.2.3; received ${releaseTag ?? "<empty>"}`);
}

const { version: wrapperVersion } = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8"));
if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(wrapperVersion)) {
  throw new Error(`Expected a stable wrapper SemVer in package.json; received ${wrapperVersion}`);
}

const appVersion = `${match[1]}.${match[2]}.${match[3]}-wrapper.${wrapperVersion}`;
const compositeTag = `v${appVersion}`;
const githubOutput = process.env.GITHUB_OUTPUT;
if (githubOutput) {
  appendFileSync(githubOutput, `app_version=${appVersion}\nrelease_tag=${compositeTag}\nwrapper_version=${wrapperVersion}\n`);
}

console.log(`SPA release: ${releaseTag}`);
console.log(`Wrapper version: ${wrapperVersion}`);
console.log(`Composite app version: ${appVersion}`);