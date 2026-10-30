param([switch]$Bench, [switch]$Web, [switch]$Window, [switch]$Android, [switch]$Audit, [string]$AndroidSerial)

$ErrorActionPreference = 'Stop'
if ($AndroidSerial -and -not $Android) { throw '-AndroidSerial requires -Android' }
if ($AndroidSerial -and $AndroidSerial -notmatch '^emulator-\d+$') { throw '-AndroidSerial requires an explicitly selected owned emulator' }
$taskBuildRoot = 'D:/BuildCache/glory-semantic-ui'
$taskWorkspace = Split-Path -Parent $PSScriptRoot
$savedEnvironment = @{}
foreach ($name in @('CARGO_HOME', 'CARGO_TARGET_DIR', 'TEMP', 'TMP', 'CARGO_PROFILE_BENCH_OPT_LEVEL', 'PYTHONPYCACHEPREFIX', 'ANDROID_USER_HOME', 'GLORY_DSL_BUILD_ROOT')) {
    $savedEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
}

Push-Location $taskWorkspace
try {
    $env:CARGO_HOME = "$taskBuildRoot/cargo"
    $env:CARGO_TARGET_DIR = "$taskBuildRoot/target"
    $env:TEMP = "$taskBuildRoot/tmp"
    $env:TMP = $env:TEMP
    $env:PYTHONPYCACHEPREFIX = "$taskBuildRoot/pycache"
    New-Item -ItemType Directory -Force -Path $env:CARGO_HOME, $env:TEMP | Out-Null
    python scripts/library_lockfiles.py
    if ($LASTEXITCODE -ne 0) { throw 'Library dependency resolution failed' }
    $metadata = cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'cargo metadata failed' }
    $actualTarget = [IO.Path]::GetFullPath($metadata.target_directory)
    $expectedTarget = [IO.Path]::GetFullPath($env:CARGO_TARGET_DIR)
    if (-not [string]::Equals($actualTarget, $expectedTarget, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Unexpected build directory: $actualTarget"
    }
    Write-Output "Verified target: $actualTarget"
    python scripts/check-current-docs.py
    if ($LASTEXITCODE -ne 0) { throw 'current documentation consistency failed' }
    if ($Audit) {
        python scripts/check-dependencies.py --out "$taskBuildRoot/dependency-audit"
        if ($LASTEXITCODE -ne 0) { throw 'dependency audit failed' }
    }
    $routingGraph = cargo tree --locked -p glory-routing --no-default-features --edges normal,build --prefix none
    if ($LASTEXITCODE -ne 0) { throw 'routing dependency graph failed' }
    if ($routingGraph | Select-String '^(glory-ui|glory-html|glory-web|glory-ssr|glory-apace|salvo|serde|serde_json|wasm-bindgen) v') {
        throw 'default routing graph acquired UI, DOM, HTTP or serialization dependencies'
    }
    $facadeGraph = cargo tree --locked -p glory --no-default-features --edges normal,build --prefix none
    if ($LASTEXITCODE -ne 0) { throw 'minimal facade dependency graph failed' }
    if ($facadeGraph | Select-String '^(glory-routing|url) v') {
        throw 'minimal facade acquired optional routing dependencies'
    }
    $toolMetadata = cargo metadata --locked --manifest-path tools/cli/Cargo.toml --no-deps --format-version 1 | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0 -or [IO.Path]::GetFullPath($toolMetadata.target_directory) -ne $expectedTarget) { throw 'Unexpected CLI target directory' }
    cargo fmt --manifest-path tools/cli/Cargo.toml -- --check
    if ($LASTEXITCODE -ne 0) { throw 'project CLI formatting failed' }
    cargo test --locked --manifest-path tools/cli/Cargo.toml
    if ($LASTEXITCODE -ne 0) { throw 'project CLI tests failed' }
    cargo clippy --locked --manifest-path tools/cli/Cargo.toml --all-targets -- -D warnings
    if ($LASTEXITCODE -ne 0) { throw 'project CLI clippy failed' }
    $toolArtifacts = cargo build --locked --manifest-path tools/cli/Cargo.toml --example bootstrap --message-format=json-render-diagnostics | ForEach-Object { $_ | ConvertFrom-Json }
    if ($LASTEXITCODE -ne 0) { throw 'project CLI acceptance driver build failed' }
    $toolDriver = $toolArtifacts | Where-Object { $_.reason -eq 'compiler-artifact' -and $_.target.name -eq 'bootstrap' -and $_.executable } | Select-Object -Last 1 -ExpandProperty executable
    if (-not $toolDriver) { throw 'Cargo did not report the CLI acceptance driver' }
    & "$taskWorkspace/tools/cli/tests/native-acceptance.ps1" -Driver $toolDriver
    if ($LASTEXITCODE -ne 0) { throw 'project CLI native acceptance failed' }
    python scripts/check-engine-forks.py --test
    if ($LASTEXITCODE -ne 0) { throw 'pinned engine fork verification/tests failed' }

    cargo fmt --all -- --check
    if ($LASTEXITCODE -ne 0) { throw 'semantic formatting failed' }
    cargo test --locked --workspace --all-features
    if ($LASTEXITCODE -ne 0) { throw 'semantic tests failed' }
    cargo test --locked -p glory-apace --no-default-features --features package,raster --test package
    if ($LASTEXITCODE -ne 0) { throw 'package-only resource tests failed' }
    $packageGraph = cargo tree --locked -p glory --no-default-features --features apace-package --edges normal,build --prefix none
    if ($LASTEXITCODE -ne 0) { throw 'package dependency graph failed' }
    if ($packageGraph | Select-String '^(reqwest|rustls|rustls-platform-verifier|cap-std|cap-fs-ext) v') {
        throw 'package-only graph acquired desktop transport dependencies'
    }
    cargo clippy --locked --workspace --all-features --all-targets -- -D warnings
    if ($LASTEXITCODE -ne 0) { throw 'semantic clippy failed' }

    if ($Window) {
        cargo test --locked -p glory-apace --features shell --lib real_windows -- --ignored --nocapture --test-threads=1
        if ($LASTEXITCODE -ne 0) { throw 'native window acceptance failed' }
        cargo test --locked -p glory-apace --features accesskit --lib real_windows -- --ignored --nocapture --test-threads=1
        if ($LASTEXITCODE -ne 0) { throw 'AccessKit native window acceptance failed' }
        cargo test --locked -p glory-apace --features accesskit --lib real_accesskit_windows_ui_automation -- --ignored --nocapture --test-threads=1
        if ($LASTEXITCODE -ne 0) { throw 'Windows UI Automation acceptance failed' }
    }

    if ($Web) {
        cargo clippy --locked --workspace --all-features --all-targets --target wasm32-unknown-unknown -- -D warnings
        if ($LASTEXITCODE -ne 0) { throw 'Web workspace clippy failed' }
        cargo fmt --manifest-path examples/semantic-web/Cargo.toml -- --check
        if ($LASTEXITCODE -ne 0) { throw 'Web fixture formatting failed' }
        cargo clippy --locked --manifest-path examples/semantic-web/Cargo.toml --target wasm32-unknown-unknown --all-targets -- -D warnings
        if ($LASTEXITCODE -ne 0) { throw 'Web fixture clippy failed' }
        & "$PSScriptRoot/build-semantic-web.ps1"
        $env:GLORY_DSL_BUILD_ROOT = "$taskBuildRoot/dsl-diagnostics"
        python scripts/check-display-diagnostics.py
        if ($LASTEXITCODE -ne 0) { throw 'downstream DSL diagnostics failed' }
    }

    if ($Bench) {
        $env:CARGO_PROFILE_BENCH_OPT_LEVEL = '3'
        cargo bench -p glory-ui --bench virtual_list | Tee-Object -FilePath "$taskBuildRoot/list-benchmark.txt"
        if ($LASTEXITCODE -ne 0) { throw 'list benchmark failed' }
    }

    if ($Android) {
        $mobileMetadata = cargo metadata --locked --manifest-path examples/mobile/Cargo.toml --no-deps --format-version 1 | ConvertFrom-Json
        if ($LASTEXITCODE -ne 0 -or [IO.Path]::GetFullPath($mobileMetadata.target_directory) -ne $expectedTarget) { throw 'Unexpected mobile target directory' }
        cargo fmt --manifest-path examples/mobile/Cargo.toml -- --check
        if ($LASTEXITCODE -ne 0) { throw 'Mobile formatting failed' }
        $mobileGraph = cargo tree --locked --manifest-path examples/mobile/Cargo.toml --target aarch64-linux-android --edges normal,build --prefix none
        if ($LASTEXITCODE -ne 0) { throw 'mobile dependency graph failed' }
        if ($mobileGraph | Select-String '^(reqwest|rustls|rustls-platform-verifier|cap-std|cap-fs-ext) v') {
            throw 'offline mobile graph acquired desktop transport dependencies'
        }
        cargo clippy --locked -p glory-apace --no-default-features --features css,assets,raster,accesskit,android-native-activity --target aarch64-linux-android --no-deps -- -D warnings
        if ($LASTEXITCODE -ne 0) { throw 'Android adapter Clippy failed' }
        & "$taskWorkspace/examples/mobile/build-android.ps1" -Target x86_64-linux-android
        & "$taskWorkspace/examples/mobile/build-android.ps1" -Target aarch64-linux-android
        if ($AndroidSerial) {
            $env:ANDROID_USER_HOME = "$taskBuildRoot/android-home"
            New-Item -ItemType Directory -Force $env:ANDROID_USER_HOME | Out-Null
            & "$env:ANDROID_HOME/platform-tools/adb.exe" -s $AndroidSerial install --no-incremental -r D:/BuildCache/glory-mobile/package-x86_64-linux-android-debug/glory-counter.apk
            if ($LASTEXITCODE -ne 0) { throw 'Android APK installation failed' }
            python examples/mobile/accept-android.py --serial $AndroidSerial --out "$taskBuildRoot/android-acceptance"
            if ($LASTEXITCODE -ne 0) { throw 'Android emulator acceptance failed' }
        } else {
            Write-Output 'Android APK builds passed; no runtime acceptance requested (use -AndroidSerial with an owned emulator).'
        }
    }
}
finally {
    Pop-Location
    foreach ($name in $savedEnvironment.Keys) {
        if ($null -eq $savedEnvironment[$name]) {
            Remove-Item -LiteralPath "Env:$name" -ErrorAction SilentlyContinue
        }
        else {
            [Environment]::SetEnvironmentVariable($name, $savedEnvironment[$name], 'Process')
        }
    }
}
