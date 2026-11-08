import assert from "node:assert/strict";
import test from "node:test";
import { validateSpaReleaseEvent } from "./validate-release-event.mjs";

test("accepts a matching stable release tag, version, and commit SHA", () => {
  assert.doesNotThrow(() =>
    validateSpaReleaseEvent({
      tag: "v1.2.3",
      version: "1.2.3",
      sha: "a".repeat(40),
    }),
  );
});

test("rejects unsupported release tags", () => {
  assert.throws(
    () =>
      validateSpaReleaseEvent({
        tag: "v1.2.3-rc.1",
        version: "1.2.3-rc.1",
        sha: "a".repeat(40),
      }),
    /Unsupported SPA release tag/,
  );
});

test("rejects mismatched versions and invalid commit SHAs", () => {
  assert.throws(
    () =>
      validateSpaReleaseEvent({
        tag: "v1.2.3",
        version: "1.2.4",
        sha: "a".repeat(40),
      }),
    /do not match/,
  );
  assert.throws(
    () =>
      validateSpaReleaseEvent({
        tag: "v1.2.3",
        version: "1.2.3",
        sha: "A".repeat(40),
      }),
    /Invalid SPA commit SHA/,
  );
});