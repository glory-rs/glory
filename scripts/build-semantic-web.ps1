param([switch]$Serve)

$ErrorActionPreference = 'Stop'
$taskBuildRoot = 'D:/BuildCache/glory-semantic-ui'
$taskWorkspace = Split-Path -Parent $PSScriptRoot
$savedEnvironment = @{}
foreach ($name in @('CARGO_HOME', 'CARGO_TARGET_DIR', 'TEMP', 'TMP', 'PYTHONPYCACHEPREFIX')) {
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
    $metadata = cargo metadata --manifest-path examples/semantic-web/Cargo.toml --no-deps --format-version 1 | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'cargo metadata failed' }
    $actual = [IO.Path]::GetFullPath($metadata.target_directory)
    if (-not [string]::Equals($actual, [IO.Path]::GetFullPath($env:CARGO_TARGET_DIR), [StringComparison]::OrdinalIgnoreCase)) { throw "Unexpected build directory: $actual" }
    Write-Output "Verified target: $actual"
    $bindgen = wasm-bindgen --version
    if ($LASTEXITCODE -ne 0 -or $bindgen.Trim() -ne 'wasm-bindgen 0.2.123') { throw 'This fixture requires wasm-bindgen-cli 0.2.123 (same as the pinned library).' }
    cargo build --locked --manifest-path examples/semantic-web/Cargo.toml --lib --target wasm32-unknown-unknown
    if ($LASTEXITCODE -ne 0) { throw 'Web fixture build failed' }
    wasm-bindgen "$taskBuildRoot/target/wasm32-unknown-unknown/debug/glory_semantic_web.wasm" --target web --out-dir "$taskBuildRoot/web-demo"
    if ($LASTEXITCODE -ne 0) { throw 'WASM bindings failed' }
    Copy-Item -LiteralPath examples/semantic-web/index.html -Destination "$taskBuildRoot/web-demo/index.html"
    cargo run --locked --manifest-path examples/semantic-web/Cargo.toml --bin ssr_fixture -- "$taskBuildRoot/web-demo"
    if ($LASTEXITCODE -ne 0) { throw 'SSR fixture generation failed' }
    Write-Output "Browser fixture: $taskBuildRoot/web-demo"
    if ($Serve) { python -m http.server 8788 --bind 127.0.0.1 --directory "$taskBuildRoot/web-demo" }
}
finally {
    Pop-Location
    foreach ($name in $savedEnvironment.Keys) {
        if ($null -eq $savedEnvironment[$name]) { Remove-Item -LiteralPath "Env:$name" -ErrorAction SilentlyContinue }
        else { [Environment]::SetEnvironmentVariable($name, $savedEnvironment[$name], 'Process') }
    }
}
