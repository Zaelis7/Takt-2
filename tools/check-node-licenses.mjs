import path from "node:path";
import process from "node:process";
import { pathToFileURL } from "node:url";

const approvedLicenses = new Set([
  "(MIT OR CC0-1.0)",
  "Apache-2.0",
  "BSD-2-Clause",
  "BSD-3-Clause",
  "BlueOak-1.0.0",
  "ISC",
  "MIT",
]);

const scopedExceptions = new Map([
  ["CC-BY-4.0", new Set(["caniuse-lite"])],
  ["MPL-2.0", new Set(["lightningcss"])],
  ["Python-2.0", new Set(["argparse"])],
]);

function isScopedException(license, packageName) {
  const approvedPackages = scopedExceptions.get(license);
  if (approvedPackages === undefined) {
    return false;
  }

  return [...approvedPackages].some(
    (approvedPackage) =>
      packageName === approvedPackage ||
      packageName.startsWith(`${approvedPackage}-`),
  );
}

export function validateLicenseReport(
  report,
  { allowScopedExceptions = true } = {},
) {
  if (report === null || typeof report !== "object" || Array.isArray(report)) {
    throw new TypeError("pnpm license report must be an object");
  }

  const violations = [];
  let packageCount = 0;
  for (const [license, packages] of Object.entries(report)) {
    if (!Array.isArray(packages)) {
      throw new TypeError(`pnpm license report entry ${license} must be an array`);
    }

    for (const packageEntry of packages) {
      const packageName = packageEntry?.name;
      if (typeof packageName !== "string" || packageName.length === 0) {
        throw new TypeError(`pnpm license report entry ${license} has no package name`);
      }

      packageCount += 1;
      if (
        !approvedLicenses.has(license) &&
        (!allowScopedExceptions || !isScopedException(license, packageName))
      ) {
        violations.push(`${license}: ${packageName}`);
      }
    }
  }

  if (violations.length > 0) {
    throw new Error(
      `Node license policy violations:\n${violations.sort().join("\n")}`,
    );
  }

  return packageCount;
}

async function readStandardInput() {
  process.stdin.setEncoding("utf8");
  let input = "";
  for await (const chunk of process.stdin) {
    input += chunk;
  }
  if (input.trim().length === 0) {
    throw new Error("pnpm produced an empty license report");
  }
  return input;
}

const invokedPath = process.argv[1];
const isMain =
  invokedPath !== undefined &&
  import.meta.url === pathToFileURL(path.resolve(invokedPath)).href;

if (isMain) {
  const report = JSON.parse(await readStandardInput());
  const packageCount = validateLicenseReport(report, {
    allowScopedExceptions: !process.argv.includes("--production"),
  });
  console.log(`Validated licenses for ${packageCount} installed Node packages`);
}
