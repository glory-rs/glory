const { expect, test } = require("@playwright/test");
const { requiredUrl } = require("./helpers");

test.skip(!process.env.GLORY_ROUTER_URL, "Set GLORY_ROUTER_URL to run this project.");

test("CSR routing navigates without a full page contract change", async ({ page }) => {
  const baseUrl = requiredUrl("GLORY_ROUTER_URL");
  await page.goto(baseUrl);

  await expect(page.getByRole("heading", { name: "Basic Router Example" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Home" })).toBeVisible();

  await page.getByRole("link", { name: "Dashboard" }).click();
  await expect(page).toHaveURL(/\/dashboard$/);
  await expect(page.getByRole("heading", { name: "Dashboard" })).toBeVisible();

  await page.getByRole("link", { name: "About" }).click();
  await expect(page).toHaveURL(/\/about$/);
  await expect(page.getByRole("heading", { name: "About" })).toBeVisible();
});
