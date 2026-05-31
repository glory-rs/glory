# Glory Tauri Template

Minimal packaging skeleton for a future `cargo glory --target desktop --package tauri`
flow.

Expected layout:

```text
src-tauri/
  tauri.conf.json
  src/main.rs
```

The desktop renderer emits `WryCommand` JSON; a Tauri shell can forward the same
commands into the webview by evaluating `WRY_INTERPRETER_JS` during startup.
