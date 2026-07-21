import { spawn } from "node:child_process";
import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { generateMessages } from "@cucumber/gherkin";
import { IdGenerator, SourceMediaType } from "@cucumber/messages";
import YAML from "yaml";

const repositoryRoot = path.resolve(import.meta.dirname, "..");
const allowedStatuses = new Set(["planned", "runnable", "verified"]);

function locator(entry) {
  return `${entry?.source}\0${entry?.name ?? entry?.scenario}`;
}

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

function requireString(value, label, errors) {
  if (typeof value !== "string" || value.trim() === "") {
    errors.push(`${label} must be a non-empty string`);
  }
}

function requireStrings(value, label, errors, { nonEmpty = false } = {}) {
  if (!Array.isArray(value) || value.some((item) => typeof item !== "string")) {
    errors.push(`${label} must be an array of strings`);
    return false;
  }
  if (nonEmpty && value.length === 0) {
    errors.push(`${label} must not be empty`);
  }
  if (value.some((item) => item.trim() === "")) {
    errors.push(`${label} must contain only non-empty strings`);
  }
  return true;
}

function sameStrings(left, right) {
  return (
    Array.isArray(left) &&
    Array.isArray(right) &&
    [...left].sort().join("\0") === [...right].sort().join("\0")
  );
}

function validateTestCommands(binding, label, errors) {
  if (!Array.isArray(binding?.tests)) {
    errors.push(`${label}.tests must be an array`);
    return;
  }
  if (binding.status !== "planned" && binding.tests.length === 0) {
    errors.push(`${label} is a runnable binding and must declare at least one test command`);
  }
  for (const [index, command] of binding.tests.entries()) {
    const commandLabel = `${label}.tests[${index}]`;
    if (command === null || typeof command !== "object" || Array.isArray(command)) {
      errors.push(`${commandLabel} must be an object`);
      continue;
    }
    requireString(command.command, `${commandLabel}.command`, errors);
    requireStrings(command.args, `${commandLabel}.args`, errors);
  }
}

export function validateAcceptanceBindings(
  scenarios,
  manifest,
  packageIds,
  { requireRunnable = false, releases = new Set() } = {},
) {
  const errors = [];
  if (manifest?.schema_version !== 1) {
    errors.push("acceptance bindings schema_version must be 1");
  }
  const bindings = manifest?.bindings;
  if (!Array.isArray(bindings)) {
    throw new Error("Acceptance bindings are invalid:\n- bindings must be an array");
  }

  const scenarioLocators = scenarios.map(locator);
  const duplicateScenarios = duplicates(scenarioLocators);
  if (duplicateScenarios.length > 0) {
    errors.push(`duplicate Gherkin scenarios: ${duplicateScenarios.join(", ")}`);
  }
  const bindingIds = bindings.map((entry) => entry?.id);
  const duplicateIds = duplicates(bindingIds);
  if (duplicateIds.length > 0) {
    errors.push(`duplicate acceptance binding IDs: ${duplicateIds.join(", ")}`);
  }
  const bindingLocators = bindings.map(locator);
  const duplicateBindings = duplicates(bindingLocators);
  if (duplicateBindings.length > 0) {
    errors.push(`duplicate acceptance scenario bindings: ${duplicateBindings.join(", ")}`);
  }

  const scenariosByLocator = new Map(scenarios.map((entry) => [locator(entry), entry]));
  const bindingsByLocator = new Map(bindings.map((entry) => [locator(entry), entry]));

  for (const scenario of scenarios) {
    if (!bindingsByLocator.has(locator(scenario))) {
      errors.push(
        `unbound acceptance scenario ${scenario.release} ${scenario.source}: ${scenario.name}`,
      );
    }
  }

  for (const binding of bindings) {
    const label = `binding ${binding?.id ?? "<missing>"}`;
    requireString(binding?.id, `${label}.id`, errors);
    requireString(binding?.release, `${label}.release`, errors);
    requireString(binding?.source, `${label}.source`, errors);
    requireString(binding?.scenario, `${label}.scenario`, errors);
    requireStrings(binding?.requirements, `${label}.requirements`, errors);
    if (
      requireStrings(
        binding?.implementation_packages,
        `${label}.implementation_packages`,
        errors,
        { nonEmpty: true },
      )
    ) {
      for (const packageId of binding.implementation_packages) {
        if (!packageIds.has(packageId)) {
          errors.push(`${label} references unknown work package ${packageId}`);
        }
      }
    }
    if (!allowedStatuses.has(binding?.status)) {
      errors.push(`${label}.status must be planned, runnable or verified`);
    }
    validateTestCommands(binding, label, errors);

    const scenario = scenariosByLocator.get(locator(binding));
    if (scenario === undefined) {
      errors.push(
        `binding ${binding?.id ?? "<missing>"} references no Gherkin scenario: ${binding?.scenario ?? "<missing>"}`,
      );
      continue;
    }
    if (binding.release !== scenario.release) {
      errors.push(`${label}.release does not match the Gherkin release tag`);
    }
    if (!sameStrings(binding.requirements, scenario.requirements)) {
      errors.push(`${label}.requirements do not match Gherkin tags`);
    }
    if (
      requireRunnable &&
      (releases.size === 0 || releases.has(scenario.release)) &&
      binding.status === "planned"
    ) {
      errors.push(
        `release ${scenario.release} scenario ${scenario.name} is planned, not runnable`,
      );
    }
  }

  if (errors.length > 0) {
    throw new Error(`Acceptance bindings are invalid:\n- ${errors.join("\n- ")}`);
  }

  return {
    planned: bindings.filter((entry) => entry.status === "planned").length,
    runnable: bindings.filter((entry) => entry.status === "runnable").length,
    verified: bindings.filter((entry) => entry.status === "verified").length,
  };
}

async function acceptanceScenarios() {
  const directory = path.join(repositoryRoot, "specs/acceptance");
  const files = (await readdir(directory))
    .filter((name) => name.endsWith(".feature"))
    .sort();
  const scenarios = [];

  for (const file of files) {
    const sourcePath = `specs/acceptance/${file}`;
    const source = await readFile(path.join(directory, file), "utf8");
    const envelopes = generateMessages(
      source,
      sourcePath,
      SourceMediaType.TEXT_X_CUCUMBER_GHERKIN_PLAIN,
      {
        includeGherkinDocument: true,
        includePickles: false,
        includeSource: false,
        newId: IdGenerator.incrementing(),
      },
    );
    const parseErrors = envelopes
      .filter((entry) => entry.parseError !== undefined)
      .map((entry) => entry.parseError.message);
    if (parseErrors.length > 0) {
      throw new Error(`${sourcePath} has Gherkin syntax errors:\n${parseErrors.join("\n")}`);
    }
    const document = envelopes.find((entry) => entry.gherkinDocument !== undefined)
      ?.gherkinDocument;
    const feature = document?.feature;
    const releases = (feature?.tags ?? [])
      .map((tag) => tag.name)
      .filter((tag) => /^@v\d+\.\d+$/.test(tag));
    if (releases.length !== 1) {
      throw new Error(`${sourcePath} must have exactly one @vMAJOR.MINOR tag`);
    }
    for (const child of feature?.children ?? []) {
      if (child.scenario === undefined) {
        continue;
      }
      scenarios.push({
        release: releases[0].slice(1),
        source: sourcePath,
        name: child.scenario.name,
        requirements: child.scenario.tags
          .map((tag) => tag.name.slice(1))
          .filter((tag) => /^PRD-[A-Z]+-\d{3}$/.test(tag)),
      });
    }
  }
  return scenarios;
}

function parseArguments(args) {
  const releases = new Set();
  let requireRunnable = false;
  let run = false;
  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === "--require-runnable") {
      requireRunnable = true;
    } else if (argument === "--run") {
      requireRunnable = true;
      run = true;
    } else if (argument === "--release") {
      index += 1;
      const release = args[index];
      if (typeof release !== "string" || release.trim() === "") {
        throw new Error("--release requires a vMAJOR.MINOR value");
      }
      releases.add(release);
    } else {
      throw new Error(`Unknown acceptance binding argument: ${argument}`);
    }
  }
  return { releases, requireRunnable, run };
}

async function execute(command) {
  await new Promise((resolve, reject) => {
    const child = spawn(command.command, command.args, {
      cwd: repositoryRoot,
      shell: false,
      stdio: "inherit",
    });
    child.once("error", reject);
    child.once("exit", (code, signal) => {
      if (code === 0) {
        resolve();
      } else {
        reject(
          new Error(
            `${command.command} ${command.args.join(" ")} failed with ${signal ?? `exit ${code}`}`,
          ),
        );
      }
    });
  });
}

async function main() {
  const options = parseArguments(process.argv.slice(2));
  const scenarios = await acceptanceScenarios();
  const manifest = YAML.parse(
    await readFile(
      path.join(repositoryRoot, "specs/acceptance/bindings.yaml"),
      "utf8",
    ),
  );
  const workPackages = YAML.parse(
    await readFile(
      path.join(repositoryRoot, "docs/implementation-tracking/work-packages.yaml"),
      "utf8",
    ),
  );
  const knownReleases = new Set(scenarios.map((entry) => entry.release));
  for (const release of options.releases) {
    if (!knownReleases.has(release)) {
      throw new Error(`Unknown acceptance release: ${release}`);
    }
  }
  const counts = validateAcceptanceBindings(
    scenarios,
    manifest,
    new Set(workPackages.packages.map((entry) => entry.id)),
    options,
  );

  if (options.run) {
    const selectedBindings = manifest.bindings.filter(
      (entry) => options.releases.size === 0 || options.releases.has(entry.release),
    );
    const commands = new Map();
    for (const binding of selectedBindings) {
      for (const command of binding.tests) {
        commands.set(JSON.stringify(command), command);
      }
    }
    for (const command of commands.values()) {
      await execute(command);
    }
    console.log(
      `Acceptance behavior passed: ${selectedBindings.length} scenarios via ${commands.size} test commands`,
    );
    return;
  }

  console.log(
    `Acceptance bindings valid: ${scenarios.length} scenarios (${counts.planned} planned, ${counts.runnable} runnable, ${counts.verified} verified)`,
  );
}

if (process.argv[1] !== undefined && pathToFileURL(process.argv[1]).href === import.meta.url) {
  try {
    await main();
  } catch (error) {
    console.error(error instanceof Error ? error.message : error);
    process.exitCode = 1;
  }
}
