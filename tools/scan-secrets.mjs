import { execFile as execFileCallback } from "node:child_process";
import { readFile } from "node:fs/promises";
import { promisify } from "node:util";
import path from "node:path";

const execFile = promisify(execFileCallback);
const repositoryRoot = path.resolve(import.meta.dirname, "..");
const { stdout } = await execFile(
  "git",
  ["ls-files", "--cached", "--others", "--exclude-standard", "-z"],
  { cwd: repositoryRoot, encoding: "buffer", maxBuffer: 16 * 1024 * 1024 },
);
const patterns = [
  {
    name: "private key",
    expression: new RegExp(
      `${"-----BEGIN"} (?:RSA |EC |OPENSSH )?PRIVATE KEY-----`,
      "u",
    ),
  },
  { name: "AWS access key", expression: /\b(?:AKIA|ASIA)[A-Z0-9]{16}\b/u },
  { name: "GitHub token", expression: /\bgh(?:p|o|u|s|r)_[A-Za-z0-9]{30,}\b/u },
  { name: "Slack token", expression: /\bxox(?:b|p|a|r|s)-[A-Za-z0-9-]{20,}\b/u },
  {
    name: "credential-bearing database URL",
    expression: /\b(?:postgres(?:ql)?|mysql):\/\/[^\s:/]+:[^\s@/]+@/u,
  },
];
const paths = stdout
  .toString("utf8")
  .split("\0")
  .filter(Boolean)
  .filter((entry) => !entry.endsWith("pnpm-lock.yaml") && !entry.endsWith("Cargo.lock"));
const findings = [];

for (const relativePath of paths) {
  const absolutePath = path.join(repositoryRoot, relativePath);
  let bytes;
  try {
    bytes = await readFile(absolutePath);
  } catch {
    continue;
  }
  if (bytes.length > 2 * 1024 * 1024 || bytes.includes(0)) {
    continue;
  }
  const text = bytes.toString("utf8");
  for (const pattern of patterns) {
    if (pattern.expression.test(text)) {
      findings.push(`${relativePath}: ${pattern.name}`);
    }
  }
}

if (findings.length > 0) {
  throw new Error(`Potential committed secrets detected:\n${findings.join("\n")}`);
}

console.log(`Secret pattern scan passed for ${paths.length} source files`);
