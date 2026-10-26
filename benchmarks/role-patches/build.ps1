param(
    [Parameter(Mandatory)][string]$SourceRoot,
    [Parameter(Mandatory)][string]$BuildRoot,
    [Parameter(Mandatory)][string]$Bindgen
)
$ErrorActionPreference = 'Stop'
foreach ($path in @($SourceRoot, $BuildRoot)) {
    if (-not [IO.Path]::GetFullPath($path).Replace('\', '/').StartsWith('D:/BuildCache/', [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Frozen source and build output must be under D:/BuildCache'
    }
}
$saved = @{}
foreach ($key in @('CARGO_HOME', 'CARGO_TARGET_DIR', 'CARGO_BUILD_JOBS', 'TEMP', 'TMP')) {
    $saved[$key] = [Environment]::GetEnvironmentVariable($key, 'Process')
}
try {
    $env:CARGO_HOME = 'D:/BuildCache/glory-semantic-ui/cargo'
    $env:CARGO_TARGET_DIR = "$BuildRoot/target"
    $env:CARGO_BUILD_JOBS = '2'
    $env:TEMP = "$BuildRoot/tmp"; $env:TMP = $env:TEMP
    New-Item -ItemType Directory -Force $env:TEMP, "$BuildRoot/perf-site" | Out-Null
    $manifest = "$SourceRoot/benchmarks/role-patches/Cargo.toml"
    $metadata = cargo metadata --manifest-path $manifest --no-deps --format-version 1 | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0 -or [IO.Path]::GetFullPath($metadata.target_directory) -ne [IO.Path]::GetFullPath($env:CARGO_TARGET_DIR)) {
        throw 'Unexpected Cargo target'
    }
    $metadata | ConvertTo-Json -Depth 100 | Set-Content "$BuildRoot/metadata.json"
    if ((& $Bindgen --version).Trim() -ne 'wasm-bindgen 0.2.129') { throw 'Requires wasm-bindgen-cli 0.2.129' }
    cargo build --locked --manifest-path $manifest --bin bench-role-patches --release --message-format=json-render-diagnostics > "$BuildRoot/native-build.jsonl" 2> "$BuildRoot/native-build.log"
    if ($LASTEXITCODE -ne 0) { throw 'Native kernel build failed' }
    cargo build --locked --manifest-path $manifest --lib --release --target wasm32-unknown-unknown --message-format=json-render-diagnostics > "$BuildRoot/wasm-build.jsonl" 2> "$BuildRoot/wasm-build.log"
    if ($LASTEXITCODE -ne 0) { throw 'WASM kernel build failed' }
    & $Bindgen "$env:CARGO_TARGET_DIR/wasm32-unknown-unknown/release/bench_role_patches.wasm" --target web --out-dir "$BuildRoot/perf-site" --out-name app
    if ($LASTEXITCODE -ne 0) { throw 'Bindings failed' }
    Copy-Item -LiteralPath "$SourceRoot/benchmarks/role-patches/index.html" -Destination "$BuildRoot/perf-site/index.html"
} finally {
    foreach ($key in $saved.Keys) { [Environment]::SetEnvironmentVariable($key, $saved[$key], 'Process') }
}
