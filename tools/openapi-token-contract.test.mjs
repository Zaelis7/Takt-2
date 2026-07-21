import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";

import YAML from "yaml";

const repositoryRoot = path.resolve(import.meta.dirname, "..");
const contract = YAML.parse(
  await readFile(path.join(repositoryRoot, "specs/contracts/openapi.yaml"), "utf8"),
);

function operation(pathName, method) {
  const value = contract.paths?.[pathName]?.[method];
  assert.ok(value, `${method.toUpperCase()} ${pathName} must be defined`);
  return value;
}

function parameterNames(value) {
  return new Set(
    (value.parameters ?? []).map((parameter) =>
      parameter.$ref?.split("/").at(-1) ?? parameter.name,
    ),
  );
}

function propertyOwners(name, value, owners = []) {
  if (!value || typeof value !== "object") return owners;
  if (value.properties && Object.hasOwn(value.properties, name)) owners.push(value);
  for (const nested of Object.values(value)) propertyOwners(name, nested, owners);
  return owners;
}

test("PRD-IAM-001 API token CRUD has explicit auth, concurrency and pagination", () => {
  const collectionPath = "/api/v1/api-tokens";
  const itemPath = "/api/v1/api-tokens/{api_token_id}";
  const list = operation(collectionPath, "get");
  const create = operation(collectionPath, "post");
  const get = operation(itemPath, "get");
  const patch = operation(itemPath, "patch");
  const revoke = operation(itemPath, "delete");

  for (const value of [list, create, get, patch, revoke]) {
    assert.deepEqual(value.security, [{ bearerAuth: [] }, { sessionCookie: [] }]);
    assert.ok(value.responses["401"]);
    assert.ok(value.responses["403"]);
  }
  assert.deepEqual(parameterNames(list), new Set([
    "Limit", "Cursor", "project_id", "kind", "status", "scope",
  ]));
  assert.match(list.description, /created_at.*id/i);
  for (const value of [create, patch, revoke]) {
    assert.ok(parameterNames(value).has("IdempotencyKey"));
    assert.ok(parameterNames(value).has("CsrfTokenIfSession"));
  }
  for (const value of [patch, revoke]) assert.ok(parameterNames(value).has("IfMatch"));

  assert.equal(list.responses["200"].content["application/json"].schema.$ref, "#/components/schemas/ApiTokenPage");
  assert.equal(create.responses["201"].content["application/json"].schema.$ref, "#/components/schemas/ApiTokenCreated");
  assert.equal(get.responses["200"].content["application/json"].schema.$ref, "#/components/schemas/ApiToken");
  assert.equal(patch.responses["200"].content["application/json"].schema.$ref, "#/components/schemas/ApiToken");
  assert.equal(revoke.responses["204"].content, undefined);
  assert.equal(create.responses["201"].headers.ETag.$ref, "#/components/headers/ETag");
  assert.ok(create.responses["201"].headers.Location);
  assert.equal(get.responses["200"].headers.ETag.$ref, "#/components/headers/ETag");
  assert.equal(patch.responses["200"].headers.ETag.$ref, "#/components/headers/ETag");
  for (const [value, status] of [[list, "200"], [create, "201"], [get, "200"], [patch, "200"], [revoke, "204"]]) {
    assert.equal(value.responses[status].headers["X-Request-Id"].$ref, "#/components/headers/RequestId");
  }
});

test("PRD-IAM-001 token value is one-time while safe schemas remain redacted", () => {
  const schemas = contract.components.schemas;
  for (const name of ["ApiTokenCreate", "ApiTokenPatch", "ApiToken", "ApiTokenCreated", "ApiTokenPage"]) {
    assert.equal(schemas[name].additionalProperties, false, `${name} must be closed`);
  }
  assert.deepEqual(schemas.ApiTokenCreated.required, ["token", "api_token"]);
  assert.equal(schemas.ApiTokenCreated.properties.token.readOnly, true);
  assert.match(schemas.ApiTokenCreated.properties.token.description, /only.*creation/i);
  assert.equal(schemas.ApiToken.properties.token, undefined);
  assert.ok(schemas.ApiToken.properties.token_prefix);
  assert.equal(schemas.ApiTokenCreate.properties.scopes.uniqueItems, true);
  assert.equal(schemas.ApiTokenCreate.properties.scopes.items.$ref, "#/components/schemas/ApiTokenScope");
  assert.equal(schemas.ApiTokenScope.pattern, "^[a-z][a-z0-9_-]*:[a-z][a-z0-9_-]*$");
  assert.equal(schemas.ApiTokenCreate.properties.ip_networks.items.$ref, "#/components/schemas/IpNetwork");
  assert.equal(schemas.ApiTokenPatch.properties.scopes, undefined, "scope expansion requires replacement");
  assert.deepEqual(schemas.ApiTokenPage.required, ["items", "next_cursor"]);

  const owners = [];
  for (const [name, schema] of Object.entries(schemas)) {
    if (propertyOwners("token", schema).length > 0) owners.push(name);
  }
  assert.deepEqual(owners.sort(), [
    "ApiTokenCreated",
    "HttpBearerAuth",
    "PasswordRecoveryComplete",
  ]);
});
