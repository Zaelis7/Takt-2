import { expect, test } from "@playwright/test";

test("bootstrap web shell has an accessible heading", async ({ page }) => {
  await page.goto("/");

  await expect(
    page.getByRole("heading", { level: 1, name: "Technical foundation" }),
  ).toBeVisible();
  await expect(page.locator("main")).toHaveAttribute("data-contract-status", "ok");
});
