param([switch]$Bench, [switch]$Web, [switch]$Window)

$ErrorActionPreference = 'Stop'
$taskBuildRoot = 'D:/BuildCache/glory-semantic-ui'
$taskWorkspace = Split-Path -Parent $PSScriptRoot
$savedEnvironment = @{}
foreach ($name in @('CARGO_HOME', 'CARGO_TARGET_DIR', 'TEMP', 'TMP', 'CARGO_PROFILE_BENCH_OPT_LEVEL', 'PYTHONPYCACHEPREFIX')) {
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
    $metadata = cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'cargo metadata failed' }
    $actualTarget = [IO.Path]::GetFullPath($metadata.target_directory)
    $expectedTarget = [IO.Path]::GetFullPath($env:CARGO_TARGET_DIR)
    if (-not [string]::Equals($actualTarget, $expectedTarget, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Unexpected build directory: $actualTarget"
    }
    Write-Output "Verified target: $actualTarget"
    python scripts/check-engine-forks.py --test
    if ($LASTEXITCODE -ne 0) { throw 'pinned engine fork verification/tests failed' }

    cargo fmt --all -- --check
    if ($LASTEXITCODE -ne 0) { throw 'semantic formatting failed' }
    cargo test --locked --workspace --all-features
    if ($LASTEXITCODE -ne 0) { throw 'semantic tests failed' }
    cargo clippy --locked --workspace --all-features --all-targets -- -D warnings
    if ($LASTEXITCODE -ne 0) { throw 'semantic clippy failed' }

    if ($Window) {
        cargo test --locked -p glory-blitz --features shell --lib real_windows -- --ignored --nocapture --test-threads=1
        if ($LASTEXITCODE -ne 0) { throw 'native window acceptance failed' }
        cargo test --locked -p glory-blitz --features accesskit --lib real_windows -- --ignored --nocapture --test-threads=1
        if ($LASTEXITCODE -ne 0) { throw 'AccessKit native window acceptance failed' }
        cargo test --locked -p glory-blitz --features accesskit --lib real_accesskit_windows_ui_automation -- --ignored --nocapture --test-threads=1
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
    }

    if ($Bench) {
        $env:CARGO_PROFILE_BENCH_OPT_LEVEL = '3'
        cargo bench -p glory-ui --bench virtual_list | Tee-Object -FilePath "$taskBuildRoot/list-benchmark.txt"
        if ($LASTEXITCODE -ne 0) { throw 'list benchmark failed' }
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
