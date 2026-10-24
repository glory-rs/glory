const { expect, test } = require("@playwright/test");
const { requiredUrl } = require("./helpers");

test.skip(!process.env.GLORY_COUNTER_URL, "Set GLORY_COUNTER_URL to run this project.");

test("shared counter updates from semantic buttons and controlled input", async ({ page }) => {
  const baseUrl = requiredUrl("GLORY_COUNTER_URL");
  const pageErrors = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));

  await page.goto(baseUrl);

  const counter = page.locator(".counter");
  await expect(counter).toContainText("Value: 0");

  await counter.getByRole("button", { name: "+1" }).click();
  await expect(counter).toContainText("Value: 1");
  await expect(counter.getByRole("textbox", { name: "Counter value" })).toHaveValue("1");

  await counter.getByRole("button", { name: "-1", exact: true }).click();
  await counter.getByRole("button", { name: "-1", exact: true }).click();
  await expect(counter).toContainText("Value: -1");

  await counter.getByRole("textbox", { name: "Counter value" }).fill("42");
  await expect(counter).toContainText("Value: 42");

  await counter.getByRole("button", { name: "Clear" }).click();
  await expect(counter).toContainText("Value: 0");
  await expect(counter.getByRole("textbox", { name: "Counter value" })).toHaveValue("0");

  await counter.getByRole("textbox", { name: "Counter value" }).fill("-");
  await expect(counter.getByRole("textbox", { name: "Counter value" })).toHaveValue("-");
  await counter.getByRole("textbox", { name: "Counter value" }).pressSequentially("8");
  await expect(counter).toContainText("Value: -8");

  for (const value of ["-8", "", "invalid", "2147483648", "0007"]) {
    await counter.getByRole("textbox", { name: "Counter value" }).fill(value);
    await counter.getByRole("textbox", { name: "Counter value" }).press("Enter");
    const expected = value === "-8" ? "-8" : value === "0007" ? "7" : "0";
    await expect(counter).toContainText(`Value: ${expected}`);
    await expect(counter.getByRole("textbox", { name: "Counter value" })).toHaveValue(expected);
  }
  await page.reload();
  await expect(page.locator(".counter")).toContainText("Value: 0");
  expect(pageErrors).toEqual([]);
});
