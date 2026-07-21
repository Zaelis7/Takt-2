import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import path from "node:path";

import { parse } from "yaml";

const repositoryRoot = path.resolve(import.meta.dirname, "..");

test("PRD-API-002 locks every js-yaml package to 4.3.0 or newer", async () => {
  const lockfileText = await readFile(
    path.join(repositoryRoot, "pnpm-lock.yaml"),
    "utf8",
  );
  const lockfile = parse(lockfileText);
  const packageKeys = Object.keys(lockfile?.packages ?? {});
  const jsYamlVersions = packageKeys
    .map((packageKey) => /^js-yaml@(\d+)\.(\d+)\.(\d+)$/.exec(packageKey))
    .filter((match) => match !== null)
    .map((match) => match.slice(1).map(Number));

  assert.ok(jsYamlVersions.length > 0, "pnpm lockfile has no js-yaml package");
  assert.deepEqual(
    jsYamlVersions.filter(
      ([major, minor]) => major < 4 || (major === 4 && minor < 3),
    ),
    [],
    "pnpm lockfile contains a js-yaml version older than 4.3.0",
  );
});
