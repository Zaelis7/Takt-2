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

function responseCodes(value) {
  return new Set(Object.keys(value.responses ?? {}));
}

test("PRD-IAM-001 browser auth operations have explicit security boundaries", () => {
  const login = operation("/api/v1/auth/login", "post");
  const logout = operation("/api/v1/auth/logout", "post");
  const session = operation("/api/v1/auth/session", "get");
  const recoveryRequest = operation("/api/v1/auth/recovery/request", "post");
  const recoveryComplete = operation("/api/v1/auth/recovery/complete", "post");

  assert.deepEqual(login.security, []);
  assert.deepEqual(recoveryRequest.security, []);
  assert.deepEqual(recoveryComplete.security, []);
  assert.deepEqual(logout.security, [{ sessionCookie: [] }]);
  assert.deepEqual(session.security, [{ sessionCookie: [] }]);
  assert.match(login.description, /identical.*unknown username.*invalid password/i);
  assert.match(recoveryRequest.description, /identical.*account exists/i);

  const sessionCookie = contract.components.headers.SessionCookie.description;
  assert.match(sessionCookie, /HttpOnly/);
  assert.match(sessionCookie, /SameSite=Lax/);
  assert.match(sessionCookie, /Path=\//);
  assert.match(sessionCookie, /Secure outside explicit localhost mode/);

  const expiredSessionCookie =
    contract.components.headers.ExpiredSessionCookie.description;
  assert.match(expiredSessionCookie, /HttpOnly/);
  assert.match(expiredSessionCookie, /SameSite=Lax/);
  assert.match(expiredSessionCookie, /Path=\//);
  assert.match(expiredSessionCookie, /Secure outside explicit localhost mode/);

  assert.ok(
    logout.parameters.some(
      (parameter) => parameter.$ref === "#/components/parameters/CsrfToken",
    ),
    "logout must require the shared CSRF header",
  );
  assert.equal(contract.components.parameters.CsrfToken.required, true);
  assert.equal(contract.components.parameters.CsrfToken.in, "header");

  assert.deepEqual(responseCodes(login), new Set(["200", "400", "401", "422", "429"]));
  assert.deepEqual(responseCodes(logout), new Set(["204", "401", "403", "429"]));
  assert.deepEqual(responseCodes(session), new Set(["200", "401", "429"]));
  assert.deepEqual(responseCodes(recoveryRequest), new Set(["202", "400", "422", "429"]));
  assert.deepEqual(responseCodes(recoveryComplete), new Set(["204", "400", "422", "429"]));

  for (const value of [login, logout, session, recoveryRequest, recoveryComplete]) {
    assert.equal(
      value.responses["429"].$ref,
      "#/components/responses/RateLimitProblem",
    );
  }

  for (const [value, success] of [
    [login, "200"],
    [logout, "204"],
    [session, "200"],
    [recoveryRequest, "202"],
    [recoveryComplete, "204"],
  ]) {
    assert.equal(
      value.responses[success].headers["X-Request-Id"].$ref,
      "#/components/headers/RequestId",
    );
  }
  assert.equal(
    contract.components.responses.Problem.headers["X-Request-Id"].$ref,
    "#/components/headers/RequestId",
  );
  assert.equal(
    contract.components.responses.RateLimitProblem.headers["Retry-After"].$ref,
    "#/components/headers/RetryAfter",
  );
  assert.equal(
    contract.components.responses.RateLimitProblem.content["application/problem+json"]
      .schema.$ref,
    "#/components/schemas/RateLimitProblem",
  );
  assert.equal(
    contract.components.schemas.RateLimitProblem.allOf[1].properties.code.const,
    "rate_limit_exceeded",
  );

  assert.equal(recoveryRequest.responses["202"].content, undefined);
  assert.equal(recoveryRequest.responses["404"], undefined);
});

test("PRD-IAM-001 auth schemas bound and redact credentials and session identifiers", () => {
  const schemas = contract.components.schemas;
  for (const name of [
    "LoginRequest",
    "PasswordRecoveryRequest",
    "PasswordRecoveryComplete",
  ]) {
    assert.equal(schemas[name].additionalProperties, false, `${name} must be closed`);
  }

  for (const password of [
    schemas.LoginRequest.properties.password,
    schemas.PasswordRecoveryComplete.properties.new_password,
  ]) {
    assert.equal(password.minLength, 12);
    assert.equal(password.maxLength, 1024);
    assert.equal(password.writeOnly, true);
    assert.match(password.description, /1024 UTF-8 bytes/);
  }

  const recoveryToken = schemas.PasswordRecoveryComplete.properties.token;
  assert.equal(recoveryToken.writeOnly, true);
  assert.equal(recoveryToken.minLength, 32);
  assert.equal(recoveryToken.maxLength, 512);

  assert.deepEqual(schemas.Session.required, [
    "user",
    "permissions",
    "csrf_token",
    "expires_at",
    "absolute_expires_at",
  ]);
  assert.equal(schemas.Session.additionalProperties, false);
  assert.equal(schemas.Session.properties.session_id, undefined);
  assert.equal(schemas.Session.properties.password, undefined);
  assert.equal(schemas.Session.properties.recovery_token, undefined);
  assert.match(schemas.Session.properties.expires_at.description, /12 hours/);
  assert.match(schemas.Session.properties.absolute_expires_at.description, /7 days/);
});
