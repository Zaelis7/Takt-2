import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

const repositoryRoot = path.resolve(import.meta.dirname, "..");
const specsRoot = path.join(repositoryRoot, "specs");

function duplicates(values) {
  const seen = new Set();
  const repeated = new Set();
  for (const value of values) {
    if (seen.has(value)) {
      repeated.add(value);
    }
    seen.add(value);
  }
  return [...repeated].sort();
}

function isUnsafe(entry) {
  return (
    entry.includes("\\") ||
    entry.startsWith("/") ||
    /^[A-Za-z]:/.test(entry) ||
    entry.split("/").includes("..")
  );
}

function globExpression(entry) {
  const escaped = entry
    .split("*")
    .map((part) => part.replace(/[.+^${}()|[\]\\]/g, "\\$&"))
    .join("[^/]*");
  return new RegExp(`^${escaped}$`);
}

export function validateSpecIndexEntries(entries, files) {
  if (!Array.isArray(entries) || entries.length === 0) {
    throw new Error("Specification index has no document entries");
  }
  if (
    entries.some((entry) => typeof entry !== "string" || entry.trim() === "") ||
    files.some((file) => typeof file !== "string")
  ) {
    throw new TypeError("Specification index entries and files must be strings");
  }

  const errors = [];
  const repeated = duplicates(entries);
  if (repeated.length > 0) {
    errors.push(`duplicate indexed paths: ${repeated.join(", ")}`);
  }
  const unsafe = entries.filter(
    (entry) => isUnsafe(entry) || /[?\[\]{}]/.test(entry),
  );
  if (unsafe.length > 0) {
    errors.push(`unsafe indexed paths: ${unsafe.sort().join(", ")}`);
  }

  const normalizedFiles = files.map((file) => file.replaceAll("\\", "/"));
  const missing = entries.filter((entry) => {
    if (unsafe.includes(entry)) {
      return false;
    }
    const expression = globExpression(entry);
    return !normalizedFiles.some((file) => expression.test(file));
  });
  if (missing.length > 0) {
    errors.push(`missing indexed paths: ${missing.sort().join(", ")}`);
  }

  if (errors.length > 0) {
    throw new Error(`Specification index is invalid:\n- ${errors.join("\n- ")}`);
  }
}

export function parseSpecIndex(source) {
  const start = source.indexOf("## 1. Dokumente");
  const end = source.indexOf("## 2. Verbindlichkeit");
  if (start < 0 || end <= start) {
    throw new Error("Specification index document section is missing");
  }
  return [...source.slice(start, end).matchAll(/^\|\s*`([^`]+)`\s*\|/gm)].map(
    (match) => match[1],
  );
}

async function filesBelow(directory) {
  const files = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const absolute = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await filesBelow(absolute)));
    } else if (entry.isFile()) {
      files.push(path.relative(specsRoot, absolute).replaceAll("\\", "/"));
    }
  }
  return files;
}

async function main() {
  const source = await readFile(path.join(specsRoot, "README.md"), "utf8");
  const entries = parseSpecIndex(source);
  const files = await filesBelow(specsRoot);
  validateSpecIndexEntries(entries, files);
  console.log(`Specification index valid: ${entries.length} indexed paths`);
}

if (process.argv[1] !== undefined && pathToFileURL(process.argv[1]).href === import.meta.url) {
  try {
    await main();
  } catch (error) {
    console.error(error instanceof Error ? error.message : error);
    process.exitCode = 1;
  }
}
