// Run against a disposable copy of the counter, never the user's source tree.
const fs = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");
const { execFileSync } = require("node:child_process");
const values = Object.fromEntries(process.argv.slice(2).reduce((pairs, value, index, args) => {
  if (index % 2 === 0) pairs.push([value.replace(/^--/, ""), args[index + 1]]);
  return pairs;
}, []));
for (const key of ["fixture", "build", "log", "out", "driver", "playwright"]) {
  if (!values[key]) throw new Error(`Missing --${key}`);
}
const normalized = path.resolve(values.fixture).replaceAll("\\", "/").toLowerCase();
if (!normalized.startsWith("d:/buildcache/")) throw new Error("Use a disposable D: fixture");
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
async function until(callback, timeout = 90000) {
  const start = Date.now();
  while (Date.now() - start < timeout) {
    try { const value = await callback(); if (value) return value; } catch {}
    await sleep(150);
  }
  throw new Error("Timed out waiting for CLI acceptance state");
}
const hash = file => crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex");

(async () => {
  const url = await until(() => fs.readFileSync(values.log, "utf8").match(/Glory: (http:\/\/127\.0\.0\.1:\d+)/)?.[1]);
  const state = () => fetch(url + "/__glory_dev/state").then(response => response.json());
  const { chromium } = require(values.playwright);
  const browser = await chromium.launch({ headless: true, channel: "chrome" });
  const page = await browser.newPage();
  const errors = [];
  page.on("pageerror", error => errors.push(error.message));
  const source = path.join(values.fixture, "src/lib.rs");
  const css = path.join(values.fixture, "public/theme.css");
  const originalSource = fs.readFileSync(source);
  const originalCss = fs.readFileSync(css);
  fs.mkdirSync(values.out, { recursive: true });
  try {
    let previous = await state();
    await sleep(1200);
    await until(async () => { const current = await state(); if (current.revision === previous.revision && !current.error) return current; previous = current; return false; });
    await page.goto(url);
    await page.waitForFunction(() => document.querySelector(".counter")?.textContent.includes("Value: 0"));
    await page.getByRole("button", { name: "+1", exact: true }).click();
    await page.waitForFunction(() => document.querySelector(".counter")?.textContent.includes("Value: 1"));
    fs.writeFileSync(css, "body { --cli-theme: updated; }\n");
    await page.waitForFunction(() => getComputedStyle(document.body).getPropertyValue("--cli-theme").trim() === "updated", null, { timeout: 90000 });
    await page.waitForFunction(() => document.querySelector(".counter")?.textContent.includes("Value: 0"));
    const manifest = path.join(values.build, "web-debug/glory-build.json");
    const acceptedHash = hash(manifest);
    const acceptedState = await state();
    fs.appendFileSync(source, '\ncompile_error!("CLI_ACCEPTANCE_BROKEN_BUILD");\n');
    await until(async () => { const current = await state(); return current.error?.includes("CLI_ACCEPTANCE_BROKEN_BUILD") && current; });
    if (hash(manifest) !== acceptedHash) throw new Error("Failed rebuild replaced the accepted output");
    await page.getByRole("button", { name: "+1", exact: true }).click();
    await page.waitForFunction(() => document.querySelector(".counter")?.textContent.includes("Value: 1"));
    const doctor = execFileSync(values.driver, ["doctor", "--manifest-path", path.join(values.fixture, "Cargo.toml"), "--build-root", values.build, "--offline"], { encoding: "utf8", windowsHide: true });
    if (!doctor.includes("Framework version")) throw new Error("CLI did not run with broken application sources");
    fs.writeFileSync(source, originalSource);
    await until(async () => { const current = await state(); return !current.error && current.revision > acceptedState.revision && current; });
    await page.waitForFunction(() => document.querySelector(".counter")?.textContent.includes("Value: 0"));
    if (errors.length) throw new Error(errors.join("\n"));
    await page.screenshot({ path: path.join(values.out, "counter.png"), fullPage: true });
    fs.writeFileSync(path.join(values.out, "result.json"), JSON.stringify({ url, cssRefresh: true, oldAppSurvivedCompileError: true, acceptedOutputPreserved: true, doctorWithBrokenApp: true, recoveryRefresh: true, pageErrors: errors }, null, 2));
    console.log("CLI browser acceptance passed", url);
  } finally {
    fs.writeFileSync(source, originalSource);
    fs.writeFileSync(css, originalCss);
    await browser.close();
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
