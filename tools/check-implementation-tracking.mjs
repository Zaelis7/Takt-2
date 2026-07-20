import { readdir, readFile, stat } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

import YAML from "yaml";

const repositoryRoot = path.resolve(import.meta.dirname, "..");
const trackingDirectory = path.join(
  repositoryRoot,
  "docs/implementation-tracking",
);

const allowed = {
  coverage: new Set(["none", "partial", "full"]),
  verification: new Set([
    "none",
    "evidence_only",
    "focused_local",
    "full_local",
    "independent",
    "ci",
    "release",
  ]),
  specHealth: new Set(["clear", "gap", "conflict", "decision_required"]),
  packageStatus: new Set([
    "planned",
    "in_progress",
    "implemented",
    "verified",
    "blocked",
  ]),
  findingStatus: new Set(["open", "resolved", "accepted"]),
  findingSeverity: new Set(["critical", "high", "medium", "low"]),
  findingKind: new Set([
    "gap",
    "conflict",
    "error",
    "ambiguity",
    "evidence_gap",
  ]),
  decision: new Set(["implementation", "spec_change", "owner_decision"]),
};

function duplicates(values) {
  const seen = new Set();
  const duplicateValues = new Set();
  for (const value of values) {
    if (seen.has(value)) {
      duplicateValues.add(value);
    }
    seen.add(value);
  }
  return [...duplicateValues].sort();
}

function requireNonEmptyString(value, label, errors) {
  if (typeof value !== "string" || value.trim() === "") {
    errors.push(`${label} must be a non-empty string`);
  }
}

function requireStringArray(value, label, errors, { nonEmpty = false } = {}) {
  if (!Array.isArray(value) || value.some((entry) => typeof entry !== "string")) {
    errors.push(`${label} must be an array of strings`);
    return;
  }
  if (nonEmpty && value.length === 0) {
    errors.push(`${label} must not be empty`);
  }
}

function dependencyCycle(packagesById) {
  const visiting = new Set();
  const visited = new Set();
  const stack = [];

  function visit(id) {
    if (visiting.has(id)) {
      const start = stack.indexOf(id);
      return [...stack.slice(start), id];
    }
    if (visited.has(id)) {
      return undefined;
    }

    visiting.add(id);
    stack.push(id);
    const entry = packagesById.get(id);
    for (const dependency of entry?.depends_on ?? []) {
      if (!packagesById.has(dependency)) {
        continue;
      }
      const cycle = visit(dependency);
      if (cycle !== undefined) {
        return cycle;
      }
    }
    stack.pop();
    visiting.delete(id);
    visited.add(id);
    return undefined;
  }

  for (const id of packagesById.keys()) {
    const cycle = visit(id);
    if (cycle !== undefined) {
      return cycle;
    }
  }
  return undefined;
}

export function validateTrackingModel(canonicalRequirements, model) {
  const errors = [];
  const requirementDocument = model.requirements ?? {};
  const packageDocument = model.workPackages ?? {};
  const findingDocument = model.findings ?? {};
  const requirements = requirementDocument.requirements ?? [];
  const packages = packageDocument.packages ?? [];
  const findings = findingDocument.findings ?? [];

  if (requirementDocument.schema_version !== 1) {
    errors.push("requirements schema_version must be 1");
  }
  if (packageDocument.schema_version !== 1) {
    errors.push("work packages schema_version must be 1");
  }
  if (findingDocument.schema_version !== 1) {
    errors.push("findings schema_version must be 1");
  }
  if (!/^[0-9a-f]{40}$/.test(requirementDocument.baseline_commit ?? "")) {
    errors.push("requirements baseline_commit must be a lowercase 40-character SHA");
  }
  if (!/^\d{4}-\d{2}-\d{2}$/.test(requirementDocument.as_of ?? "")) {
    errors.push("requirements as_of must use YYYY-MM-DD");
  }
  if (!Array.isArray(requirements)) {
    errors.push("requirements must be an array");
  }
  if (!Array.isArray(packages)) {
    errors.push("packages must be an array");
  }
  if (!Array.isArray(findings)) {
    errors.push("findings must be an array");
  }

  const requirementIds = requirements.map((entry) => entry?.id);
  const duplicateRequirementIds = duplicates(requirementIds);
  if (duplicateRequirementIds.length > 0) {
    errors.push(`duplicate requirement IDs: ${duplicateRequirementIds.join(", ")}`);
  }
  const missingRequirements = [...canonicalRequirements]
    .filter((id) => !requirementIds.includes(id))
    .sort();
  if (missingRequirements.length > 0) {
    errors.push(`missing canonical requirements: ${missingRequirements.join(", ")}`);
  }
  const unknownLedgerRequirements = requirementIds
    .filter((id) => !canonicalRequirements.has(id))
    .sort();
  if (unknownLedgerRequirements.length > 0) {
    errors.push(
      `unknown requirements in ledger: ${unknownLedgerRequirements.join(", ")}`,
    );
  }

  for (const entry of requirements) {
    const label = `requirement ${entry?.id ?? "<missing>"}`;
    requireNonEmptyString(entry?.release, `${label}.release`, errors);
    if (!allowed.coverage.has(entry?.coverage)) {
      errors.push(`${label}.coverage is invalid`);
    }
    if (!allowed.verification.has(entry?.verification)) {
      errors.push(`${label}.verification is invalid`);
    }
    if (!allowed.specHealth.has(entry?.spec_health)) {
      errors.push(`${label}.spec_health is invalid`);
    }
    requireStringArray(entry?.evidence, `${label}.evidence`, errors);
    requireNonEmptyString(entry?.note, `${label}.note`, errors);
    if (entry?.coverage === "none" && entry?.verification !== "none") {
      errors.push(`${label} cannot be verified while coverage is none`);
    }
    if (
      entry?.coverage !== "none" &&
      Array.isArray(entry?.evidence) &&
      entry.evidence.length === 0
    ) {
      errors.push(`${label} needs evidence for non-zero coverage`);
    }
  }

  const findingIds = findings.map((entry) => entry?.id);
  const duplicateFindingIds = duplicates(findingIds);
  if (duplicateFindingIds.length > 0) {
    errors.push(`duplicate finding IDs: ${duplicateFindingIds.join(", ")}`);
  }
  const findingsById = new Map(findings.map((entry) => [entry.id, entry]));
  for (const entry of findings) {
    const label = `finding ${entry?.id ?? "<missing>"}`;
    requireNonEmptyString(entry?.id, `${label}.id`, errors);
    if (!allowed.findingStatus.has(entry?.status)) {
      errors.push(`${label}.status is invalid`);
    }
    if (!allowed.findingSeverity.has(entry?.severity)) {
      errors.push(`${label}.severity is invalid`);
    }
    if (!allowed.findingKind.has(entry?.kind)) {
      errors.push(`${label}.kind is invalid`);
    }
    if (!allowed.decision.has(entry?.decision)) {
      errors.push(`${label}.decision is invalid`);
    }
    requireNonEmptyString(entry?.summary, `${label}.summary`, errors);
    requireStringArray(entry?.locations, `${label}.locations`, errors, {
      nonEmpty: true,
    });
    requireStringArray(entry?.affects, `${label}.affects`, errors);
    requireNonEmptyString(entry?.resolution, `${label}.resolution`, errors);
    for (const requirement of entry?.affects ?? []) {
      if (!canonicalRequirements.has(requirement)) {
        errors.push(`${label} references unknown requirement ${requirement}`);
      }
    }
  }

  const packageIds = packages.map((entry) => entry?.id);
  const duplicatePackageIds = duplicates(packageIds);
  if (duplicatePackageIds.length > 0) {
    errors.push(`duplicate package IDs: ${duplicatePackageIds.join(", ")}`);
  }
  const packagesById = new Map(packages.map((entry) => [entry.id, entry]));
  const mappedRequirements = new Set();
  for (const entry of packages) {
    const label = `package ${entry?.id ?? "<missing>"}`;
    requireNonEmptyString(entry?.id, `${label}.id`, errors);
    requireNonEmptyString(entry?.release, `${label}.release`, errors);
    if (!allowed.packageStatus.has(entry?.status)) {
      errors.push(`${label}.status is invalid`);
    }
    requireStringArray(entry?.requirements, `${label}.requirements`, errors);
    requireStringArray(entry?.findings, `${label}.findings`, errors);
    requireStringArray(entry?.depends_on, `${label}.depends_on`, errors);
    requireNonEmptyString(entry?.outcome, `${label}.outcome`, errors);
    requireStringArray(entry?.acceptance, `${label}.acceptance`, errors, {
      nonEmpty: true,
    });
    requireStringArray(entry?.evidence, `${label}.evidence`, errors, {
      nonEmpty: true,
    });

    for (const requirement of entry?.requirements ?? []) {
      mappedRequirements.add(requirement);
      if (!canonicalRequirements.has(requirement)) {
        errors.push(`${label} references unknown requirement ${requirement}`);
      }
    }
    for (const finding of entry?.findings ?? []) {
      if (!findingsById.has(finding)) {
        errors.push(`${label} references unknown finding ${finding}`);
      }
    }
    for (const dependency of entry?.depends_on ?? []) {
      if (!packagesById.has(dependency)) {
        errors.push(`${label} references unknown dependency ${dependency}`);
      }
      if (dependency === entry.id) {
        errors.push(`${label} depends on itself`);
      }
    }
  }

  const unmappedRequirements = [...canonicalRequirements]
    .filter((id) => !mappedRequirements.has(id))
    .sort();
  if (unmappedRequirements.length > 0) {
    errors.push(
      `canonical requirements without a work package: ${unmappedRequirements.join(", ")}`,
    );
  }

  const cycle = dependencyCycle(packagesById);
  if (cycle !== undefined) {
    errors.push(`package dependency cycle: ${cycle.join(" -> ")}`);
  }

  const exceptions = requirementDocument.known_unknown_requirement_refs ?? [];
  if (!Array.isArray(exceptions)) {
    errors.push("known_unknown_requirement_refs must be an array");
  } else {
    for (const entry of exceptions) {
      const label = `unknown requirement exception ${entry?.id ?? "<missing>"}`;
      requireNonEmptyString(entry?.id, `${label}.id`, errors);
      requireNonEmptyString(entry?.finding, `${label}.finding`, errors);
      requireStringArray(entry?.paths, `${label}.paths`, errors, {
        nonEmpty: true,
      });
      if (canonicalRequirements.has(entry?.id)) {
        errors.push(`${label} is canonical and must not be excepted`);
      }
      const finding = findingsById.get(entry?.finding);
      if (finding === undefined) {
        errors.push(`${label} references unknown finding ${entry?.finding}`);
      } else if (finding.status !== "open") {
        errors.push(`${label} must reference an open finding`);
      }
    }
  }

  if (errors.length > 0) {
    throw new Error(`Implementation tracking is invalid:\n- ${errors.join("\n- ")}`);
  }
}

async function parseYaml(file) {
  return YAML.parse(await readFile(file, "utf8"));
}

async function existingFile(relativePath) {
  try {
    return (await stat(path.join(repositoryRoot, relativePath))).isFile();
  } catch {
    return false;
  }
}

async function sourceFiles(directory) {
  const files = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const absolute = path.join(directory, entry.name);
    const relative = path.relative(repositoryRoot, absolute).replaceAll("\\", "/");
    if (
      entry.isDirectory() &&
      ["node_modules", "target", "dist", "generated", "implementation-tracking"].includes(
        entry.name,
      )
    ) {
      continue;
    }
    if (entry.isDirectory()) {
      files.push(...(await sourceFiles(absolute)));
      continue;
    }
    if (
      entry.isFile() &&
      [".md", ".rs", ".sql", ".ts", ".tsx", ".yaml", ".yml", ".feature"].includes(
        path.extname(entry.name),
      )
    ) {
      files.push({ absolute, relative });
    }
  }
  return files;
}

async function validateRepositoryReferences(canonicalRequirements, requirements) {
  const roots = [
    "AGENTS.md",
    "crates",
    "docs",
    "migrations",
    "specs",
    "tests",
    "web/src",
  ];
  const files = [];
  for (const root of roots) {
    const absolute = path.join(repositoryRoot, root);
    const metadata = await stat(absolute);
    if (metadata.isFile()) {
      files.push({ absolute, relative: root });
    } else {
      files.push(...(await sourceFiles(absolute)));
    }
  }

  const exceptions = new Map();
  for (const entry of requirements.known_unknown_requirement_refs ?? []) {
    exceptions.set(entry.id, new Set(entry.paths));
  }
  const errors = [];
  const observedExceptions = new Map();

  for (const file of files) {
    const source = await readFile(file.absolute, "utf8");
    const ids = new Set(source.match(/PRD-[A-Z]+-[0-9]{3}/g) ?? []);
    for (const id of ids) {
      if (canonicalRequirements.has(id)) {
        continue;
      }
      const allowedPaths = exceptions.get(id);
      if (allowedPaths?.has(file.relative)) {
        const seen = observedExceptions.get(id) ?? new Set();
        seen.add(file.relative);
        observedExceptions.set(id, seen);
      } else {
        errors.push(`${file.relative} references unknown requirement ${id}`);
      }
    }
  }

  for (const [id, paths] of exceptions) {
    for (const expectedPath of paths) {
      if (!observedExceptions.get(id)?.has(expectedPath)) {
        errors.push(
          `stale unknown-requirement exception ${id} for ${expectedPath}`,
        );
      }
    }
  }
  if (errors.length > 0) {
    throw new Error(`Requirement references are invalid:\n- ${errors.join("\n- ")}`);
  }
}

async function main() {
  const productRequirements = await readFile(
    path.join(repositoryRoot, "specs/00-product-requirements.md"),
    "utf8",
  );
  const canonicalRequirements = new Set(
    productRequirements.match(/PRD-[A-Z]+-[0-9]{3}/g) ?? [],
  );
  const model = {
    requirements: await parseYaml(path.join(trackingDirectory, "requirements.yaml")),
    workPackages: await parseYaml(path.join(trackingDirectory, "work-packages.yaml")),
    findings: await parseYaml(path.join(trackingDirectory, "findings.yaml")),
  };

  validateTrackingModel(canonicalRequirements, model);

  const referencedFiles = new Set();
  for (const requirement of model.requirements.requirements) {
    for (const evidence of requirement.evidence) {
      referencedFiles.add(evidence);
    }
  }
  for (const finding of model.findings.findings) {
    for (const location of finding.locations) {
      referencedFiles.add(location);
    }
  }
  const missingFiles = [];
  for (const file of referencedFiles) {
    if (!(await existingFile(file))) {
      missingFiles.push(file);
    }
  }
  if (missingFiles.length > 0) {
    throw new Error(
      `Tracking references missing files:\n- ${missingFiles.sort().join("\n- ")}`,
    );
  }

  await validateRepositoryReferences(
    canonicalRequirements,
    model.requirements,
  );

  console.log(
    `Implementation tracking valid: ${canonicalRequirements.size} requirements, ${model.workPackages.packages.length} packages, ${model.findings.findings.length} findings`,
  );
}

if (process.argv[1] !== undefined && pathToFileURL(process.argv[1]).href === import.meta.url) {
  await main();
}
