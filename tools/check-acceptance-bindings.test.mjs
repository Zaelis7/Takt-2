import assert from "node:assert/strict";
import test from "node:test";

import { validateAcceptanceBindings } from "./check-acceptance-bindings.mjs";

const scenario = {
  release: "v0.1",
  source: "specs/acceptance/v0.1.feature",
  name: "A release behavior",
  requirements: ["PRD-TEST-001"],
};

function validManifest() {
  return {
    schema_version: 1,
    bindings: [
      {
        id: "v0.1-release-behavior",
        release: "v0.1",
        source: "specs/acceptance/v0.1.feature",
        scenario: "A release behavior",
        requirements: ["PRD-TEST-001"],
        implementation_packages: ["TEST-001"],
        status: "planned",
        tests: [],
      },
    ],
  };
}

test("accepts an exact planned binding without claiming product execution", () => {
  const result = validateAcceptanceBindings(
    [scenario],
    validManifest(),
    new Set(["TEST-001"]),
  );

  assert.deepEqual(result, {
    planned: 1,
    runnable: 0,
    verified: 0,
  });
});

test("rejects an intentionally unbound release scenario", () => {
  const manifest = validManifest();
  manifest.bindings = [];

  assert.throws(
    () =>
      validateAcceptanceBindings(
        [scenario],
        manifest,
        new Set(["TEST-001"]),
      ),
    /unbound acceptance scenario.*A release behavior/s,
  );
});

test("rejects requirement-tag drift between Gherkin and the manifest", () => {
  const manifest = validManifest();
  manifest.bindings[0].requirements = ["PRD-OTHER-001"];

  assert.throws(
    () =>
      validateAcceptanceBindings(
        [scenario],
        manifest,
        new Set(["TEST-001"]),
      ),
    /requirements do not match Gherkin tags/,
  );
});

test("rejects a runnable binding without an executable test command", () => {
  const manifest = validManifest();
  manifest.bindings[0].status = "runnable";

  assert.throws(
    () =>
      validateAcceptanceBindings(
        [scenario],
        manifest,
        new Set(["TEST-001"]),
      ),
    /runnable binding.*must declare at least one test command/s,
  );
});

test("release readiness rejects planned bindings", () => {
  assert.throws(
    () =>
      validateAcceptanceBindings(
        [scenario],
        validManifest(),
        new Set(["TEST-001"]),
        { requireRunnable: true, releases: new Set(["v0.1"]) },
      ),
    /release v0\.1 scenario.*is planned, not runnable/s,
  );
});
