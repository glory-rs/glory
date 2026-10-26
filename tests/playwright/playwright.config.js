const { defineConfig, devices } = require("@playwright/test");

const desktop = devices["Desktop Chrome"];

module.exports = defineConfig({
  testDir: "./tests",
  fullyParallel: true,
  timeout: 30_000,
  expect: {
    timeout: 10_000,
  },
  reporter: process.env.CI ? [["github"], ["list"]] : "list",
  use: {
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
    video: "retain-on-failure",
  },
  projects: [
    {
      name: "counter",
      testMatch: "counter.spec.js",
      use: desktop,
    },
  ],
});
