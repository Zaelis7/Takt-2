import { execFile as execFileCallback } from "node:child_process";
import { readFile } from "node:fs/promises";
import { promisify } from "node:util";
import path from "node:path";

const execFile = promisify(execFileCallback);
const repositoryRoot = path.resolve(import.meta.dirname, "..");
const { stdout } = await execFile(
  "cargo",
  ["metadata", "--format-version", "1", "--no-deps", "--locked"],
  { cwd: repositoryRoot },
);
const metadata = JSON.parse(stdout);
const workspaceIds = new Set(metadata.workspace_members);
const workspacePackages = metadata.packages.filter((entry) =>
  workspaceIds.has(entry.id),
);
const workspaceNames = new Set(workspacePackages.map((entry) => entry.name));
const allowedInternalDependencies = new Map([
  ["takt-api", new Set()],
  ["takt-domain", new Set()],
  ["takt-probe-protocol", new Set()],
  ["takt-server", new Set(["takt-api"])],
  ["xtask", new Set()],
]);

for (const packageMetadata of workspacePackages) {
  const allowed = allowedInternalDependencies.get(packageMetadata.name);
  if (allowed === undefined) {
    throw new Error(
      `Workspace crate ${packageMetadata.name} is missing from the architecture allow-list`,
    );
  }

  const internalDependencies = packageMetadata.dependencies
    .map((dependency) => dependency.name)
    .filter((name) => workspaceNames.has(name));
  const forbidden = internalDependencies.filter((name) => !allowed.has(name));
  if (forbidden.length > 0) {
    throw new Error(
      `${packageMetadata.name} has forbidden workspace dependencies: ${forbidden.join(", ")}`,
    );
  }

  for (const target of packageMetadata.targets) {
    const source = await readFile(target.src_path, "utf8");
    if (!source.startsWith("#![forbid(unsafe_code)]")) {
      throw new Error(
        `${path.relative(repositoryRoot, target.src_path)} must start with #![forbid(unsafe_code)]`,
      );
    }
  }
}

const domain = workspacePackages.find((entry) => entry.name === "takt-domain");
if (domain === undefined) {
  throw new Error("takt-domain is missing from the workspace");
}

const forbiddenDomainFrameworks = new Set([
  "axum",
  "reqwest",
  "sqlx",
  "tokio",
  "tonic",
  "tower",
]);
const importedFrameworks = domain.dependencies
  .map((dependency) => dependency.name)
  .filter((name) => forbiddenDomainFrameworks.has(name));
if (importedFrameworks.length > 0) {
  throw new Error(
    `takt-domain imports forbidden I/O or runtime frameworks: ${importedFrameworks.join(", ")}`,
  );
}

console.log("Workspace dependency directions and unsafe-code guards are valid");

