import { readFile } from "node:fs/promises";
import path from "node:path";

import Ajv2020 from "ajv/dist/2020.js";
import addFormats from "ajv-formats";
import YAML from "yaml";

const repositoryRoot = path.resolve(import.meta.dirname, "..");
const schemaPath = path.join(
  repositoryRoot,
  "specs/contracts/takt-config.schema.json",
);
const examplePath = path.join(repositoryRoot, "specs/examples/takt.yaml");

const schema = JSON.parse(await readFile(schemaPath, "utf8"));
const example = YAML.parse(await readFile(examplePath, "utf8"));
const validator = new Ajv2020({
  allErrors: true,
  strict: true,
  // Draft 2020-12 permits a conditional `required` to reference a property
  // declared in the containing schema; AJV's optional lint rejects that shape.
  strictRequired: false,
});
addFormats(validator);

const validate = validator.compile(schema);
if (!validate(example)) {
  throw new Error(
    `Example config does not satisfy takt-config.schema.json:\n${JSON.stringify(
      validate.errors,
      null,
      2,
    )}`,
  );
}

console.log("Validated specs/examples/takt.yaml against takt-config.schema.json");
