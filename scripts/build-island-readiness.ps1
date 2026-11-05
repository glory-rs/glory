param(
    [string]$BuildRoot = "$PSScriptRoot/../target/verification/glory-semantic-ui",
    [string]$Bindgen = 'wasm-bindgen',
    [switch]$Verify
)
. "$PSScriptRoot/local-build-paths.ps1"

$ErrorActionPreference = 'Stop'
$BuildRoot = [IO.Path]::GetFullPath($BuildRoot)
$saved = @{}
foreach ($key in @('CARGO_HOME', 'CARGO_TARGET_DIR', 'TEMP', 'TMP', 'CARGO_INCREMENTAL', 'CARGO_PROFILE_DEV_DEBUG', 'GLORY_ISLAND_SITE')) { $saved[$key] = [Environment]::GetEnvironmentVariable($key, 'Process') }
try {

    $env:CARGO_TARGET_DIR = Get-NormalCargoTarget

    $env:CARGO_INCREMENTAL = '0'
    $env:CARGO_PROFILE_DEV_DEBUG = '0'
    $env:GLORY_ISLAND_SITE = "$BuildRoot/island-readiness-site"
    New-Item -ItemType Directory -Force $env:TEMP, $env:GLORY_ISLAND_SITE | Out-Null
    $manifest = [IO.Path]::GetFullPath("$PSScriptRoot/../examples/display-web/Cargo.toml")
    $metadata = cargo metadata --locked --manifest-path $manifest --no-deps --format-version 1 | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0 -or [IO.Path]::GetFullPath($metadata.target_directory) -ne [IO.Path]::GetFullPath($env:CARGO_TARGET_DIR)) { throw 'Unexpected Cargo target' }
    cargo build --locked --manifest-path $manifest --bin island-readiness --target wasm32-unknown-unknown
    if ($LASTEXITCODE -ne 0) { throw 'Island WASM fixture failed' }
    if ((& $Bindgen --version).Trim() -ne 'wasm-bindgen 0.2.130') { throw 'Expected wasm-bindgen 0.2.130' }
    & $Bindgen "$env:CARGO_TARGET_DIR/wasm32-unknown-unknown/debug/island-readiness.wasm" --target web --out-dir $env:GLORY_ISLAND_SITE --out-name app
    if ($LASTEXITCODE -ne 0) { throw 'Island bindgen failed' }
    if ($Verify) {
        node "$PSScriptRoot/../examples/display-web/island-readiness-acceptance.cjs"
        if ($LASTEXITCODE -ne 0) { throw 'Island browser acceptance failed' }
    }
    Write-Output "Island readiness fixture: $env:GLORY_ISLAND_SITE"
} finally {
    foreach ($key in $saved.Keys) { [Environment]::SetEnvironmentVariable($key, $saved[$key], 'Process') }
}
