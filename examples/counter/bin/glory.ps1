param([Parameter(ValueFromRemainingArguments=$true)][string[]]$Arguments)
$ErrorActionPreference = 'Stop'
$project = Split-Path -Parent $PSScriptRoot
Push-Location $project
try {
    $offline = @()
    foreach ($argument in $Arguments) {
        if ($argument -eq '--') { break }
        if ($argument -eq '--offline') { $offline = @('--offline') }
    }
    $meta = cargo metadata @offline --no-deps --format-version 1 --manifest-path "$project/tools/glory/Cargo.toml" | ConvertFrom-Json
    if ($LASTEXITCODE) { throw 'Project tool metadata failed' }
    Write-Host "Cargo target: $($meta.target_directory)"
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
} finally { Pop-Location }
