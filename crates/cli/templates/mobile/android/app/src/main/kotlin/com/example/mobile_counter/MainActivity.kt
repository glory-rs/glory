// Replace the package to match WRY_ANDROID_PACKAGE. The WryActivity base
// class (plus RustWebView, Ipc, ...) is generated into this directory by
// wry's build script during `cargo ndk build` — see the cargoNdk task in
// app/build.gradle.kts.
package com.example.mobile_counter

class MainActivity : WryActivity()
