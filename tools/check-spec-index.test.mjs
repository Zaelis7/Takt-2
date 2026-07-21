import assert from "node:assert/strict";
import test from "node:test";

import { validateSpecIndexEntries } from "./check-spec-index.mjs";

const indexedPaths = [
  "00-product-requirements.md",
  "contracts/openapi.yaml",
  "acceptance/*.feature",
];
const existingFiles = [
  "00-product-requirements.md",
  "contracts/openapi.yaml",
  "acceptance/v0.1.feature",
];

test("accepts existing literal paths and a matched feature glob", () => {
  assert.doesNotThrow(() =>
    validateSpecIndexEntries(indexedPaths, existingFiles),
  );
});

test("rejects an intentionally missing indexed path", () => {
  assert.throws(
    () =>
      validateSpecIndexEntries(
        [...indexedPaths, "AGENTS.template.md"],
        existingFiles,
      ),
    /missing indexed paths: AGENTS\.template\.md/,
  );
});

test("rejects an indexed glob without a matching file", () => {
  assert.throws(
    () =>
      validateSpecIndexEntries(
        ["acceptance/*.feature"],
        ["acceptance/README.md"],
      ),
    /missing indexed paths: acceptance\/\*\.feature/,
  );
});

test("rejects index paths that escape the specification package", () => {
  assert.throws(
    () => validateSpecIndexEntries(["../AGENTS.md"], existingFiles),
    /unsafe indexed paths: \.\.\/AGENTS\.md/,
  );
});
