import { readdir, readFile } from "node:fs/promises";
import path from "node:path";

import { generateMessages } from "@cucumber/gherkin";
import { IdGenerator, SourceMediaType } from "@cucumber/messages";

const repositoryRoot = path.resolve(import.meta.dirname, "..");
const acceptanceDirectory = path.join(repositoryRoot, "specs/acceptance");
const featureFiles = (await readdir(acceptanceDirectory))
  .filter((name) => name.endsWith(".feature"))
  .sort();

if (featureFiles.length === 0) {
  throw new Error("No Gherkin feature files found in specs/acceptance");
}

const parseFailures = [];
for (const file of featureFiles) {
  const uri = `specs/acceptance/${file}`;
  const source = await readFile(path.join(acceptanceDirectory, file), "utf8");
  const envelopes = generateMessages(
    source,
    uri,
    SourceMediaType.TEXT_X_CUCUMBER_GHERKIN_PLAIN,
    {
      includeGherkinDocument: true,
      includePickles: false,
      includeSource: false,
      newId: IdGenerator.incrementing(),
    },
  );

  for (const envelope of envelopes) {
    if (envelope.parseError !== undefined) {
      parseFailures.push(`${uri}: ${envelope.parseError.message}`);
    }
  }
}

if (parseFailures.length > 0) {
  throw new Error(`Gherkin syntax errors:\n${parseFailures.join("\n")}`);
}

console.log(`Parsed ${featureFiles.length} Gherkin feature files`);
