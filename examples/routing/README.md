# Portable routing example

The same page factory runs in CSR, synchronous SSR/hydration and a native
WindowApp. The shell editor retains identity across child-parameter changes.

Build browser artifacts using D: caches and wasm-bindgen-cli 0.2.129:

```powershell
cargo build --locked --features web --lib --target wasm32-unknown-unknown
wasm-bindgen target/verification/glory-semantic-ui/target/wasm32-unknown-unknown/debug/glory_routing_demo.wasm --target web --out-dir target/verification/glory-routing/site
cargo run --locked --features ssr --bin routing-ssr -- target/verification/glory-routing/site
node browser.cjs
```

The browser script starts its own loopback server and uses the repository's
installed Playwright. Set GLORY_CHROME to an existing Chrome executable if needed.
It checks CSR/SSR adoption, dirty editor identity/selection, query/hash, history,
duplicate-URL scroll restoration, cancellation of superseded anchor retries,
CSR initial fragments with delayed WASM, raw anchors, fallback and cleanup.
It also installs explicit allow/block/pending async guards after CSR and SSR
adoption, then clicks the Rust `link_with` controls to verify supersession,
retained editors and cancellation without a late callback on teardown.

```powershell
cargo run --locked --features native --bin routing-native
```

The native entry uses the existing Wingy event loop on desktop/iOS. Android
applications build the same WindowApp using their NativeActivity/GameActivity
entry and attach_router; an application-owned AndroidApp is required.
NativeNavigation can also be used without Wingy or Apace.
