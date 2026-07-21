import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";

import Ajv2020 from "ajv/dist/2020.js";
import addFormats from "ajv-formats";
import YAML from "yaml";

const root = path.resolve(import.meta.dirname, "..");
const readJson = async (relativePath) =>
  JSON.parse(await readFile(path.join(root, relativePath), "utf8"));

const [openapi, configSchema, proto, validFixtures, invalidFixtures] =
  await Promise.all([
    readFile(path.join(root, "specs/contracts/openapi.yaml"), "utf8").then(
      YAML.parse,
    ),
    readJson("specs/contracts/takt-config.schema.json"),
    readFile(path.join(root, "specs/contracts/probe.proto"), "utf8"),
    readJson("specs/contracts/fixtures/check-spec-valid.json"),
    readJson("specs/contracts/fixtures/check-spec-invalid.json"),
  ]);

const ajv = new Ajv2020({ allErrors: true, strict: false });
addFormats(ajv);
const validateApi = ajv.compile({
  $ref: "#/components/schemas/CheckSpec",
  components: openapi.components,
});
const validateConfig = ajv.compile(configSchema);

const protoMessages = {
  http: "HttpCheck",
  tcp: "TcpCheck",
  dns: "DnsCheck",
  icmp: "IcmpCheck",
  tls: "TlsCheck",
  push: "PushCheck",
  browser: "BrowserCheck",
};

function configDocument(kind, target) {
  return {
    apiVersion: "takt.dev/v1alpha1",
    kind: "TaktProject",
    metadata: { organization: "acme", project: "prod", source: "test:golden" },
    spec: {
      monitors: [{ slug: `${kind}-golden`, name: kind, type: kind, target }],
    },
  };
}

function protoFields(messageName) {
  const start = proto.indexOf(`message ${messageName} {`);
  assert.notEqual(start, -1, `missing Proto message ${messageName}`);
  const bodyStart = proto.indexOf("{", start) + 1;
  let depth = 1;
  let end = bodyStart;
  while (depth > 0 && end < proto.length) {
    if (proto[end] === "{") depth += 1;
    if (proto[end] === "}") depth -= 1;
    end += 1;
  }
  const body = proto.slice(bodyStart, end - 1);
  return new Set(
    [...body.matchAll(/^\s*(?:repeated\s+|optional\s+)?(?:map<[^>]+>|[\w.<>]+)\s+(\w+)\s*=\s*\d+/gm)].map(
      ([, field]) => field,
    ),
  );
}

function protoFixtureShapeIsDeclared(fixture) {
  const fields = protoFields(protoMessages[fixture.kind]);
  for (const key of Object.keys(fixture.proto)) {
    if (key === "secret_body") {
      assert.match(proto, /SecretValueRef\s+secret_body\s*=\s*5/);
      continue;
    }
    assert.ok(fields.has(key), `${fixture.kind} Proto is missing ${key}`);
  }
}

const proxiedCheckKinds = new Set(["http", "tcp", "tls", "browser"]);
const resolverUriPattern = /^(?:udp|tcp|tls):\/\/(?![^/?#]*@)[^/?#]+\/?$/;
const proxyUriPattern = /^(?:http|https|socks5):\/\/(?![^/?#]*@)[^/?#]+\/?$/;

function protoNetworkOptionsAreSemanticallyValid(kind, value) {
  if (
    value.address_family !== undefined &&
    ![
      "ADDRESS_FAMILY_AUTO",
      "ADDRESS_FAMILY_IPV4",
      "ADDRESS_FAMILY_IPV6",
    ].includes(value.address_family)
  ) {
    return false;
  }
  if (
    value.resolver !== undefined &&
    (typeof value.resolver !== "string" ||
      !resolverUriPattern.test(value.resolver))
  ) {
    return false;
  }
  if (value.proxy === undefined) {
    return true;
  }
  if (!proxiedCheckKinds.has(kind)) {
    return false;
  }
  if (
    typeof value.proxy.url !== "string" ||
    !proxyUriPattern.test(value.proxy.url)
  ) {
    return false;
  }
  if (value.proxy.auth === undefined) {
    return true;
  }
  return (
    !JSON.stringify(value.proxy.auth).includes("literal") &&
    Object.keys(value.proxy.auth).sort().join(",") === "password,username"
  );
}

function assertExactFields(actual, expected, label) {
  assert.deepEqual(
    [...actual].sort(),
    [...expected].sort(),
    `${label} fields drifted from its golden fixture`,
  );
}

function protoFixtureIsSemanticallyValid({ kind, proto: value }) {
  if (value.auth !== undefined && JSON.stringify(value.auth).includes("literal")) {
    return false;
  }
  if (!protoNetworkOptionsAreSemanticallyValid(kind, value)) {
    return false;
  }
  const portValid = value.port === undefined || (value.port >= 1 && value.port <= 65535);
  switch (kind) {
    case "http":
      return (
        typeof value.url === "string" &&
        value.url.length > 0 &&
        (value.response_body_limit_bytes ?? 1048576) <= 1048576 &&
        (value.max_response_time_ms === undefined ||
          (value.max_response_time_ms >= 1 && value.max_response_time_ms <= 300000))
      );
    case "tcp":
    case "tls":
      return typeof value.host === "string" && value.host.length > 0 && portValid;
    case "dns":
      return (
        typeof value.name === "string" &&
        ["A", "AAAA", "CNAME", "MX", "TXT", "NS", "SOA", "CAA"].includes(
          value.record_type,
        )
      );
    case "icmp":
      return (
        typeof value.host === "string" &&
        (value.packets ?? 3) >= 1 &&
        (value.packets ?? 3) <= 5
      );
    case "push":
      return (value.grace_ms ?? 60000) <= 86400000;
    case "browser":
      const steps = Array.isArray(value.steps) ? value.steps : [];
      return (
        typeof value.start_url === "string" &&
        !JSON.stringify(value.steps).includes("JAVASCRIPT") &&
        steps.every(
          (step) =>
            step.action !== "ACTION_FILL" || !("literal" in step),
        )
      );
    default:
      return false;
  }
}

test("PRD-MON-002 valid CheckSpec golden fixtures match OpenAPI, config, and Proto", () => {
  assert.deepEqual(
    validFixtures.map(({ kind }) => kind),
    ["http", "tcp", "dns", "icmp", "tls", "push", "browser"],
  );
  for (const fixture of validFixtures) {
    assert.equal(validateApi(fixture.api), true, JSON.stringify(validateApi.errors));
    assert.equal(
      validateConfig(configDocument(fixture.kind, fixture.config_target)),
      true,
      JSON.stringify(validateConfig.errors),
    );
    protoFixtureShapeIsDeclared(fixture);
    assert.equal(protoFixtureIsSemanticallyValid(fixture), true);

    const apiSchema = openapi.components.schemas[protoMessages[fixture.kind] + "Spec"];
    assertExactFields(Object.keys(apiSchema.properties), Object.keys(fixture.api), `${fixture.kind} OpenAPI`);
    const configName = `${fixture.kind}Target`;
    assertExactFields(
      Object.keys(configSchema.$defs[configName].properties),
      Object.keys(fixture.config_target),
      `${fixture.kind} config`,
    );
    const protoExpected = new Set(Object.keys(fixture.proto));
    if (fixture.kind === "http") protoExpected.add("literal_body");
    assertExactFields(protoFields(protoMessages[fixture.kind]), protoExpected, `${fixture.kind} Proto`);
  }
});

test("PRD-MON-002 invalid CheckSpec golden fixtures fail every contract boundary", () => {
  for (const fixture of invalidFixtures) {
    assert.equal(validateApi(fixture.api), false, `${fixture.name}: OpenAPI accepted`);
    assert.equal(
      validateConfig(configDocument(fixture.kind, fixture.config_target)),
      false,
      `${fixture.name}: config schema accepted`,
    );
    protoFixtureShapeIsDeclared(fixture);
    assert.equal(
      protoFixtureIsSemanticallyValid(fixture),
      false,
      `${fixture.name}: Proto semantics accepted`,
    );
  }
});

test("PRD-MON-002 CheckSpec defaults and limits are canonical", () => {
  const api = openapi.components.schemas;
  const config = configSchema.$defs;
  const matchingDefaults = [
    [api.HttpCheckSpec.properties.method.default, config.httpTarget.properties.method.default],
    [api.HttpCheckSpec.properties.expected_status_min.default, config.httpTarget.properties.expectedStatus.properties.min.default],
    [api.HttpCheckSpec.properties.expected_status_max.default, config.httpTarget.properties.expectedStatus.properties.max.default],
    [api.HttpCheckSpec.properties.follow_redirects.default, config.httpTarget.properties.followRedirects.default],
    [api.HttpCheckSpec.properties.verify_tls.default, config.httpTarget.properties.verifyTls.default],
    [api.HttpCheckSpec.properties.http_version.default, config.httpTarget.properties.httpVersion.default],
    [api.HttpCheckSpec.properties.response_body_limit_bytes.default, config.httpTarget.properties.responseBodyLimitBytes.default],
    [api.DnsCheckSpec.properties.expected_rcode.default, config.dnsTarget.properties.expectedRcode.default],
    [api.DnsCheckSpec.properties.minimum_answers.default, config.dnsTarget.properties.minimumAnswers.default],
    [api.DnsCheckSpec.properties.value_match.default, config.dnsTarget.properties.valueMatch.default],
    [api.IcmpCheckSpec.properties.packets.default, config.icmpTarget.properties.packets.default],
    [api.IcmpCheckSpec.properties.required_successes.default, config.icmpTarget.properties.requiredSuccesses.default],
    [api.TlsCheckSpec.properties.port.default, config.tlsTarget.properties.port.default],
    [api.TlsCheckSpec.properties.warning_days.default, config.tlsTarget.properties.warningDays.default],
    [api.TlsCheckSpec.properties.critical_days.default, config.tlsTarget.properties.criticalDays.default],
    [api.PushCheckSpec.properties.allow_get.default, config.pushTarget.properties.allowGet.default],
    [api.BrowserCheckSpec.properties.max_network_response_bytes.default, config.browserTarget.properties.maxNetworkResponseBytes.default],
    [api.BrowserCheckSpec.properties.screenshot_on_failure_max_bytes.default, config.browserTarget.properties.screenshotOnFailureMaxBytes.default],
  ];
  for (const [apiDefault, configDefault] of matchingDefaults) {
    assert.equal(apiDefault, configDefault);
  }

  assert.equal(api.PushCheckSpec.properties.grace_ms.default, 60_000);
  assert.equal(config.pushTarget.properties.grace.default, "60s");
  assert.equal(api.HttpCheckSpec.properties.response_body_limit_bytes.maximum, 1_048_576);
  assert.equal(config.httpTarget.properties.responseBodyLimitBytes.maximum, 1_048_576);
  assert.equal(api.BrowserCheckSpec.properties.max_network_response_bytes.maximum, 10_485_760);
  assert.equal(config.browserTarget.properties.maxNetworkResponseBytes.maximum, 10_485_760);
  for (const declaration of [
    "optional uint32 follow_redirects",
    "optional bool verify_tls",
    "optional uint32 minimum_answers",
    "optional uint32 critical_days",
    "optional uint32 grace_ms",
    "optional uint32 screenshot_on_failure_max_bytes",
    "HTTP_VERSION_AUTO = 0",
    "DNS_VALUE_MATCH_CONTAINS = 0",
  ]) {
    assert.ok(proto.includes(declaration), `Proto default/presence drift: ${declaration}`);
  }
});

test("PRD-API-002 and PRD-MON-002 common network options are canonical", () => {
  const api = openapi.components.schemas;
  const config = configSchema.$defs;
  const schemaNames = {
    http: ["HttpCheckSpec", "httpTarget"],
    tcp: ["TcpCheckSpec", "tcpTarget"],
    dns: ["DnsCheckSpec", "dnsTarget"],
    icmp: ["IcmpCheckSpec", "icmpTarget"],
    tls: ["TlsCheckSpec", "tlsTarget"],
    browser: ["BrowserCheckSpec", "browserTarget"],
  };

  for (const [kind, [apiName, configName]] of Object.entries(schemaNames)) {
    assert.equal(
      api[apiName].properties.resolver.$ref,
      "#/components/schemas/ResolverUri",
      `${kind} OpenAPI resolver drift`,
    );
    assert.equal(
      config[configName].properties.resolver.$ref,
      "#/$defs/resolverUri",
      `${kind} config resolver drift`,
    );
    assert.equal(api[apiName].properties.address_family.$ref, "#/components/schemas/AddressFamily");
    assert.equal(config[configName].properties.addressFamily.$ref, "#/$defs/addressFamily");
    if (proxiedCheckKinds.has(kind)) {
      assert.equal(api[apiName].properties.proxy.$ref, "#/components/schemas/ProxySpec");
      assert.equal(config[configName].properties.proxy.$ref, "#/$defs/proxySpec");
    } else {
      assert.equal(api[apiName].properties.proxy, undefined);
      assert.equal(config[configName].properties.proxy, undefined);
    }
  }

  assert.equal(api.ResolverUri.pattern, config.resolverUri.pattern);
  assert.deepEqual(api.AddressFamily.enum, config.addressFamily.enum);
  assert.equal(api.AddressFamily.default, config.addressFamily.default);
  assertExactFields(Object.keys(api.ProxySpec.properties), Object.keys(config.proxySpec.properties), "proxy");
  assert.equal(api.ProxySpec.properties.url.pattern, config.proxySpec.properties.url.pattern);
  assertExactFields(
    Object.keys(api.ProxyBasicAuth.properties),
    Object.keys(config.proxyBasicAuth.properties),
    "proxy auth",
  );
  for (const field of ["username", "password"]) {
    assert.equal(api.ProxyBasicAuth.properties[field].$ref, "#/components/schemas/SecretRef");
    assert.equal(config.proxyBasicAuth.properties[field].$ref, "#/$defs/secretRef");
  }
  assert.equal(api.PushCheckSpec.properties.proxy, undefined);
  assert.equal(api.PushCheckSpec.properties.resolver, undefined);
  assert.equal(api.PushCheckSpec.properties.address_family, undefined);
  assert.equal(config.pushTarget.properties.proxy, undefined);
  assert.equal(config.pushTarget.properties.resolver, undefined);
  assert.equal(config.pushTarget.properties.addressFamily, undefined);

  for (const declaration of [
    "ADDRESS_FAMILY_AUTO = 0",
    "ADDRESS_FAMILY_IPV4 = 1",
    "ADDRESS_FAMILY_IPV6 = 2",
    "message ProxyBasicAuth",
    "message ProxyOptions",
  ]) {
    assert.ok(proto.includes(declaration), `Proto network option drift: ${declaration}`);
  }
});
