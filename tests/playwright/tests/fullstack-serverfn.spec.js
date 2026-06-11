const { expect, test } = require("@playwright/test");
const { requiredUrl } = require("./helpers");

test.skip(!process.env.GLORY_FULLSTACK_URL, "Set GLORY_FULLSTACK_URL to run this project.");

test("fullstack TodoMVC calls server functions for list, add, toggle, and clear", async ({ page }) => {
  const baseUrl = requiredUrl("GLORY_FULLSTACK_URL");
  await page.goto(baseUrl);

  await expect(page.getByRole("heading", { name: "todos" })).toBeVisible();
  await expect(page.locator(".status")).toContainText("synced");
  await expect(page.locator(".todo-list li")).toHaveCount(2);

  const title = `Playwright task ${Date.now()}`;
  const input = page.locator(".new-todo");
  await input.fill(title);
  await input.dispatchEvent("change");

  const row = page.locator(".todo-list li").filter({ hasText: title });
  await expect(row).toBeVisible();
  await row.locator("input.toggle").check();
  await expect(row).toHaveClass(/completed/);

  await page.getByRole("button", { name: "Clear completed" }).click();
  await expect(row).toHaveCount(0);
});
