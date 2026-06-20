const { expect, test } = require("@playwright/test");
const { requiredUrl } = require("./helpers");

test.skip(!process.env.GLORY_LIVEVIEW_URL, "Set GLORY_LIVEVIEW_URL to run this project.");

test("liveview drives the mounted widget over a websocket round-trip", async ({ page }) => {
  const baseUrl = requiredUrl("GLORY_LIVEVIEW_URL");
  await page.goto(baseUrl);

  // The server-rendered shell injects the LiveView client connect bootstrap.
  const html = await page.content();
  expect(html).toContain("__gloryLiveViewConnect");

  const increase = page.getByRole("button", { name: "+", exact: true });
  const decrease = page.getByRole("button", { name: "-", exact: true });
  await expect(increase).toBeVisible();
  await expect(decrease).toBeVisible();

  // Clicking is dispatched to the server over the websocket, which patches the
  // DOM back — so the visible body text must change after the round-trip.
  const before = await page.locator("body").innerText();
  await increase.click();
  await expect(async () => {
    const after = await page.locator("body").innerText();
    expect(after).not.toEqual(before);
  }).toPass();
});
