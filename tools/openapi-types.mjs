import { mkdir, readFile, writeFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";
import path from "node:path";
import process from "node:process";

import openapiTS, { astToString } from "openapi-typescript";

const repositoryRoot = path.resolve(import.meta.dirname, "..");
const contractPath = path.join(repositoryRoot, "specs/contracts/openapi.yaml");
const generatedPath = path.join(
  repositoryRoot,
  "web/src/generated/openapi.ts",
);

async function renderTypes() {
  const nodes = await openapiTS(pathToFileURL(contractPath));
  return astToString(nodes);
}

async function generate() {
  await mkdir(path.dirname(generatedPath), { recursive: true });
  await writeFile(generatedPath, await renderTypes(), "utf8");
}

async function check() {
  const expected = await renderTypes();
  const committed = await readFile(generatedPath, "utf8").catch((error) => {
    throw new Error(
      `Cannot read generated OpenAPI types at ${generatedPath}: ${String(error)}`,
    );
  });

  if (committed !== expected) {
    throw new Error(
      "Generated OpenAPI types drifted; run `pnpm generate:openapi`.",
    );
  }
}

const command = process.argv[2];

if (command === "generate") {
  await generate();
} else if (command === "check") {
  await check();
} else {
  throw new Error("Usage: node tools/openapi-types.mjs <generate|check>");
}
