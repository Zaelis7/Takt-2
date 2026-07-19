import assert from "node:assert/strict";
import test from "node:test";

import { validateLicenseReport } from "./check-node-licenses.mjs";

test("accepts approved licenses and scoped tooling exceptions", () => {
  const packageCount = validateLicenseReport({
    MIT: [{ name: "react" }],
    "MPL-2.0": [{ name: "lightningcss-win32-x64-msvc" }],
    "Python-2.0": [{ name: "argparse" }],
  });

  assert.equal(packageCount, 3);
});

test("rejects an unapproved license", () => {
  assert.throws(
    () =>
      validateLicenseReport({
        "GPL-3.0-only": [{ name: "unexpected-runtime" }],
      }),
    /GPL-3\.0-only: unexpected-runtime/,
  );
});

test("rejects an exception license on an unapproved package", () => {
  assert.throws(
    () =>
      validateLicenseReport({
        "MPL-2.0": [{ name: "unexpected-mpl-package" }],
      }),
    /MPL-2\.0: unexpected-mpl-package/,
  );
});

test("rejects tooling exceptions in the production dependency graph", () => {
  assert.throws(
    () =>
      validateLicenseReport(
        {
          "MPL-2.0": [{ name: "lightningcss" }],
        },
        { allowScopedExceptions: false },
      ),
    /MPL-2\.0: lightningcss/,
  );
});
