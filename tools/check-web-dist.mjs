import { exec as execCallback } from "node:child_process";
import { mkdtemp, readFile, readdir, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { promisify } from "node:util";

const exec = promisify(execCallback);
const repositoryRoot = path.resolve(import.meta.dirname, "..");
const committedDirectory = path.join(repositoryRoot, "web/dist");
const temporaryDirectory = await mkdtemp(path.join(os.tmpdir(), "takt-web-dist-"));

async function listFiles(directory, prefix = "") {
  const entries = await readdir(path.join(directory, prefix), {
    withFileTypes: true,
  });
  const files = [];
  for (const entry of entries) {
    const relativePath = path.join(prefix, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await listFiles(directory, relativePath)));
    } else if (entry.isFile()) {
      files.push(relativePath.replaceAll("\\", "/"));
    }
  }
  return files.sort();
}

try {
  await exec(
    `pnpm --dir web exec vite build --outDir "${temporaryDirectory}" --emptyOutDir`,
    { cwd: repositoryRoot },
  );

  const expectedFiles = await listFiles(temporaryDirectory);
  const committedFiles = await listFiles(committedDirectory).catch(() => []);
  if (JSON.stringify(committedFiles) !== JSON.stringify(expectedFiles)) {
    throw new Error(
      `Embedded web assets drifted. Expected [${expectedFiles.join(", ")}], committed [${committedFiles.join(", ")}]. Run \`pnpm build\`.`,
    );
  }

  for (const file of expectedFiles) {
    const expected = await readFile(path.join(temporaryDirectory, file));
    const committed = await readFile(path.join(committedDirectory, file));
    if (!expected.equals(committed)) {
      throw new Error(`Embedded web asset drifted: web/dist/${file}`);
    }
  }
} finally {
  await rm(temporaryDirectory, { force: true, recursive: true });
}

console.log("Embedded web production assets are reproducible and current");
