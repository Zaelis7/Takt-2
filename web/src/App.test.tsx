import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { App } from "./App";

describe("bootstrap application shell", () => {
  it("renders a named main heading without a browser or network", () => {
    const markup = renderToStaticMarkup(<App />);

    expect(markup).toContain("<main");
    expect(markup).toContain("Technical foundation");
    expect(markup).toContain('data-contract-status="ok"');
  });
});

