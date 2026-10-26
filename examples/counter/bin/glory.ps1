param([Parameter(ValueFromRemainingArguments=$true)][string[]]$Arguments)
$ErrorActionPreference = 'Stop'
$project = Split-Path -Parent $PSScriptRoot
$hash = [Security.Cryptography.SHA256]::Create()
try { $identity = [BitConverter]::ToString($hash.ComputeHash([Text.Encoding]::UTF8.GetBytes([IO.Path]::GetFullPath("$project/Cargo.toml")))).Replace('-','').Substring(0,16).ToLowerInvariant() } finally { $hash.Dispose() }
$build = if ($env:GLORY_BUILD_ROOT) { [IO.Path]::GetFullPath($env:GLORY_BUILD_ROOT) } else { "D:/BuildCache/glory-projects/$identity" }
if (-not $build.Replace('\','/').StartsWith('D:/BuildCache/',[StringComparison]::OrdinalIgnoreCase)) { throw 'GLORY_BUILD_ROOT must be under D:/BuildCache' }
$saved = @{}
foreach ($name in @('CARGO_HOME','CARGO_TARGET_DIR','TEMP','TMP','GLORY_BUILD_ROOT')) { $saved[$name] = [Environment]::GetEnvironmentVariable($name,'Process') }
Push-Location $project
try {
    $env:GLORY_BUILD_ROOT = $build
    if (-not $env:CARGO_HOME -or -not $env:CARGO_HOME.Replace('\','/').StartsWith('D:/BuildCache/',[StringComparison]::OrdinalIgnoreCase)) { $env:CARGO_HOME = "$build/cargo" }
    $env:CARGO_TARGET_DIR = "$build/tools-target"
    $env:TEMP = "$build/tmp"; $env:TMP = $env:TEMP
    $offline = @()
    if ($Arguments -contains '--offline') { $offline = @('--offline') }
    foreach ($path in @($env:CARGO_HOME,$env:CARGO_TARGET_DIR,$env:TEMP)) {
        $ancestor = [IO.Path]::GetFullPath($path)
        while ($ancestor) {
            if (Test-Path -LiteralPath $ancestor) {
                if ((Get-Item -LiteralPath $ancestor -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Cache paths must not pass through a junction/symlink: $ancestor" }
            }
            $ancestor = Split-Path -Parent $ancestor
        }
    }
    New-Item -ItemType Directory -Force $env:CARGO_HOME,$env:CARGO_TARGET_DIR,$env:TEMP | Out-Null
    $meta = cargo metadata @offline --no-deps --format-version 1 --manifest-path "$project/tools/glory/Cargo.toml" | ConvertFrom-Json
    if ($LASTEXITCODE) { throw 'Project tool metadata failed' }
    if ([IO.Path]::GetFullPath($meta.target_directory) -ne [IO.Path]::GetFullPath($env:CARGO_TARGET_DIR)) { throw 'Unexpected Cargo target' }
    $hostLine = rustc -vV | Where-Object { $_.StartsWith('host: ') }
    if ($LASTEXITCODE -or -not $hostLine) { throw 'Cannot determine the Rust host' }
    $native = $hostLine.Substring(6)
    $locked = @('--locked')
    if (-not (Test-Path -LiteralPath "$project/Cargo.lock") -or -not (Select-String -LiteralPath "$project/Cargo.lock" -Pattern '^name = "glory-cli"$' -Quiet)) {
        Write-Host 'Resolving the project CLI into Cargo.lock for the first time; commit that lockfile.'
        $locked = @()
    }
    cargo run @locked @offline --manifest-path "$project/tools/glory/Cargo.toml" --target $native -- @Arguments
    if ($LASTEXITCODE) { throw "Glory command exited with $LASTEXITCODE" }
}
finally {
    Pop-Location
    foreach ($name in $saved.Keys) { [Environment]::SetEnvironmentVariable($name,$saved[$name],'Process') }
}
