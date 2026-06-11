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
      name: "web-csr-counter",
      testMatch: "counter.spec.js",
      use: desktop,
    },
    {
      name: "web-csr-routing",
      testMatch: "routing.spec.js",
      use: desktop,
    },
    {
      name: "ssr-hydration",
      testMatch: "ssr-hydration.spec.js",
      use: desktop,
    },
    {
      name: "fullstack-serverfn",
      testMatch: "fullstack-serverfn.spec.js",
      use: desktop,
    },
    {
      name: "hot-reload",
      testMatch: "hot-reload.spec.js",
      use: desktop,
    },
  ],
});
