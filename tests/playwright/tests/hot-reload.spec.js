const { expect, test } = require("@playwright/test");
const { requiredUrl } = require("./helpers");

test.skip(!process.env.GLORY_HOT_RELOAD_URL, "Set GLORY_HOT_RELOAD_URL to run this project.");

test("serve mode injects the live reload websocket client", async ({ page }) => {
  const baseUrl = requiredUrl("GLORY_HOT_RELOAD_URL");
  await page.goto(baseUrl);

  const html = await page.content();
  expect(html).toContain("new WebSocket");
  expect(html).toContain("/live_reload");
});
