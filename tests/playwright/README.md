# Glory Playwright E2E

This directory contains first-party browser tests for the scenarios tracked in
`_todos.md` F2/F3. Tests are URL-driven so they can run against locally started
examples, CI-managed servers, or `glory end2end`.

Install once:

```powershell
npm --prefix tests/playwright install --package-lock=false
npm --prefix tests/playwright exec playwright install chromium
```

List projects:

```powershell
npm --prefix tests/playwright run list
```

Run a project by pointing it at a running app:

```powershell
$env:GLORY_COUNTER_URL = "http://127.0.0.1:8080"
npm --prefix tests/playwright run test:counter
```

CI currently starts the CSR counter example and the SSR hydration example, then
runs the `web-csr-counter` and `ssr-hydration` projects in Chromium.

Environment variables:

- `GLORY_COUNTER_URL`: CSR counter example.
- `GLORY_ROUTER_URL`: CSR `router-basic` example.
- `GLORY_SSR_URL`: SSR + hydration example such as `ssr-simple-salvo`.
- `GLORY_FULLSTACK_URL`: fullstack server-function example such as
  `todomvc-fullstack`.
- `GLORY_HOT_RELOAD_URL`: any app served through `glory serve` with reload
  enabled.

When an environment variable is missing, that project skips its tests. This
keeps the suite safe to install and list before CI provisions browser servers.
