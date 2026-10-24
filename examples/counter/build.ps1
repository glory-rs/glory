param(
    [ValidateSet('web', 'desktop')][string]$Platform = 'web',
    [switch]$Run,
    [string]$BuildRoot = 'D:/BuildCache/glory-semantic-ui',
    [string]$Bindgen = 'wasm-bindgen'
)

$ErrorActionPreference = 'Stop'
$taskBuildRoot = [IO.Path]::GetFullPath($BuildRoot)
if (-not $taskBuildRoot.Replace('\', '/').StartsWith('D:/BuildCache/', [StringComparison]::OrdinalIgnoreCase)) {
    throw 'BuildRoot must be inside D:/BuildCache'
}
$savedEnvironment = @{}
foreach ($name in @('CARGO_HOME', 'CARGO_TARGET_DIR', 'TEMP', 'TMP', 'PYTHONPYCACHEPREFIX')) {
    $savedEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
}

Push-Location $PSScriptRoot
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
    if (-not [string]::Equals($actualTarget, [IO.Path]::GetFullPath($env:CARGO_TARGET_DIR), [StringComparison]::OrdinalIgnoreCase)) {
        throw "Unexpected build directory: $actualTarget"
    }
    Write-Output "Verified target: $actualTarget"

    if ($Platform -eq 'web') {
        $version = & $Bindgen --version
        if ($LASTEXITCODE -ne 0 -or $version.Trim() -ne 'wasm-bindgen 0.2.129') {
            throw 'Use wasm-bindgen-cli 0.2.129 to match Cargo.lock; pass its executable with -Bindgen. See README.md.'
        }
        cargo build --locked --lib --features web --target wasm32-unknown-unknown
        if ($LASTEXITCODE -ne 0) { throw 'Counter Web build failed' }
        $webRoot = "$taskBuildRoot/counter-web"
        & $Bindgen "$env:CARGO_TARGET_DIR/wasm32-unknown-unknown/debug/counter.wasm" --target web --out-dir $webRoot
        if ($LASTEXITCODE -ne 0) { throw 'Counter Web bindings failed' }
        Copy-Item -LiteralPath index.html -Destination "$webRoot/index.html"
        Write-Output "Web output: $webRoot"
        if ($Run) {
            Write-Output 'Counter: http://127.0.0.1:8790'
            python -m http.server 8790 --bind 127.0.0.1 --directory $webRoot
            if ($LASTEXITCODE -ne 0) { throw 'Counter HTTP server failed' }
        }
    }
    else {
        cargo build --locked --bin counter-desktop --features desktop
        if ($LASTEXITCODE -ne 0) { throw 'Counter desktop build failed' }
        if ($Run) {
            & "$env:CARGO_TARGET_DIR/debug/counter-desktop.exe"
            if ($LASTEXITCODE -ne 0) { throw 'Counter desktop exited with an error' }
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
