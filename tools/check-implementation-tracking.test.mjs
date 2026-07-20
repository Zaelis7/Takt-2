import assert from "node:assert/strict";
import test from "node:test";

import { validateTrackingModel } from "./check-implementation-tracking.mjs";

const canonicalRequirements = new Set(["PRD-ONE-001", "PRD-TWO-001"]);

function validModel() {
  return {
    requirements: {
      schema_version: 1,
      baseline_commit: "a".repeat(40),
      as_of: "2026-07-20",
      requirements: [
        {
          id: "PRD-ONE-001",
          release: "0.1",
          coverage: "full",
          verification: "focused_local",
          spec_health: "clear",
          evidence: ["docs/evidence.md"],
          note: "Implemented.",
        },
        {
          id: "PRD-TWO-001",
          release: "0.2",
          coverage: "none",
          verification: "none",
          spec_health: "gap",
          evidence: [],
          note: "Blocked by a specification gap.",
        },
      ],
      known_unknown_requirement_refs: [],
    },
    workPackages: {
      schema_version: 1,
      packages: [
        {
          id: "ONE-001",
          release: "0.1",
          status: "implemented",
          requirements: ["PRD-ONE-001"],
          findings: [],
          depends_on: [],
          outcome: "A bounded result.",
          acceptance: ["A reproducible assertion."],
          evidence: ["Exact test output."],
        },
        {
          id: "TWO-001",
          release: "0.2",
          status: "blocked",
          requirements: ["PRD-TWO-001"],
          findings: ["SPEC-001"],
          depends_on: ["ONE-001"],
          outcome: "A later result.",
          acceptance: ["A reproducible assertion."],
          evidence: ["Exact test output."],
        },
      ],
    },
    findings: {
      schema_version: 1,
      findings: [
        {
          id: "SPEC-001",
          status: "open",
          severity: "high",
          kind: "gap",
          decision: "spec_change",
          summary: "A documented specification gap.",
          locations: ["specs/00-product-requirements.md"],
          affects: ["PRD-TWO-001"],
          resolution: "Add a normative contract and acceptance path.",
        },
      ],
    },
  };
}

test("accepts a complete requirement ledger and an acyclic package graph", () => {
  assert.doesNotThrow(() =>
    validateTrackingModel(canonicalRequirements, validModel()),
  );
});

test("rejects a missing canonical requirement", () => {
  const model = validModel();
  model.requirements.requirements.pop();

  assert.throws(
    () => validateTrackingModel(canonicalRequirements, model),
    /missing canonical requirements: PRD-TWO-001/,
  );
});

test("rejects unknown requirement IDs and package dependency cycles", () => {
  const model = validModel();
  model.workPackages.packages[0].requirements.push("PRD-UNKNOWN-001");
  model.workPackages.packages[0].depends_on.push("TWO-001");

  assert.throws(
    () => validateTrackingModel(canonicalRequirements, model),
    /unknown requirement PRD-UNKNOWN-001.*dependency cycle/s,
  );
});

test("rejects unknown requirement exceptions after their finding is resolved", () => {
  const model = validModel();
  model.requirements.known_unknown_requirement_refs.push({
    id: "PRD-UNKNOWN-001",
    finding: "SPEC-001",
    paths: ["docs/historical-evidence.md"],
  });
  model.findings.findings[0].status = "resolved";

  assert.throws(
    () => validateTrackingModel(canonicalRequirements, model),
    /unknown requirement exception PRD-UNKNOWN-001 must reference an open finding/,
  );
});
