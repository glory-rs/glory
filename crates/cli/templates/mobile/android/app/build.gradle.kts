// Glory Android host — app module.
//
// Replace `com.example.mobile_counter` / `mobile_counter` with your
// reversed domain + crate name everywhere in this template (they must
// match the `tao::android_binding!(com_example, mobile_counter, ...)`
// invocation in your Rust lib).

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "com.example.mobile_counter"
    compileSdk = 34

    defaultConfig {
        applicationId = "com.example.mobile_counter"
        minSdk = 24
        targetSdk = 34
        versionCode = 1
        versionName = "0.1.0"
    }

    sourceSets["main"].jniLibs.srcDirs("src/main/jniLibs")

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }
}

dependencies {
    implementation("androidx.appcompat:appcompat:1.7.0")
    implementation("androidx.lifecycle:lifecycle-process:2.8.4")
    implementation("com.google.android.material:material:1.12.0")
}

// Builds the Rust cdylib into jniLibs before packaging. wry's build
// script generates the Kotlin glue (WryActivity, RustWebView, ...) into
// WRY_ANDROID_KOTLIN_FILES_OUT_DIR with the {{package}} token replaced.
val cargoNdk by tasks.registering(Exec::class) {
    workingDir = file("../../")  // the Rust crate root
    environment("WRY_ANDROID_PACKAGE", "com.example.mobile_counter")
    environment("WRY_ANDROID_LIBRARY", "mobile_counter")
    environment(
        "WRY_ANDROID_KOTLIN_FILES_OUT_DIR",
        file("src/main/kotlin/com/example/mobile_counter").absolutePath,
    )
    commandLine(
        "cargo", "ndk",
        "-t", "arm64-v8a",
        "-o", file("src/main/jniLibs").absolutePath,
        "build", "--lib", "--release", "--features", "mobile", "--target-dir", "target/mobile",
    )
}

tasks.named("preBuild") { dependsOn(cargoNdk) }
