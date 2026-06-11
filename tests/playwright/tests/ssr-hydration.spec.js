const { expect, test } = require("@playwright/test");
const { requiredUrl } = require("./helpers");

test.skip(!process.env.GLORY_SSR_URL, "Set GLORY_SSR_URL to run this project.");

test("SSR page renders initial HTML and hydrates route clicks", async ({ page }) => {
  const baseUrl = requiredUrl("GLORY_SSR_URL");
  await page.goto(baseUrl);

  await expect(page.getByRole("heading", { name: "Basic Router Example" })).toBeVisible();
  await expect(page.locator("body")).toContainText("This example demonstrates a basic router");

  await page.getByRole("link", { name: "Dashboard" }).click();
  await expect(page).toHaveURL(/\/dashboard$/);
  await expect(page.getByRole("heading", { name: "Dashboard" })).toBeVisible();
});
