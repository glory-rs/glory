const { expect, test } = require("@playwright/test");
const { requiredUrl } = require("./helpers");

test.skip(!process.env.GLORY_COUNTER_URL, "Set GLORY_COUNTER_URL to run this project.");

test("counter CSR updates from buttons and input", async ({ page }) => {
  const baseUrl = requiredUrl("GLORY_COUNTER_URL");
  await page.goto(baseUrl);

  const counter = page.locator(".counter");
  await expect(counter).toContainText("Value: 0");

  await counter.getByRole("button", { name: "+1" }).click();
  await expect(counter).toContainText("Value: 1");

  await counter.locator("input").fill("42");
  await expect(counter).toContainText("Value: 42");

  await counter.getByRole("button", { name: "Clear" }).click();
  await expect(counter).toContainText("Value: 0");
});
