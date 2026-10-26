param(
    [Parameter(Mandatory=$true)][string]$Driver,
    [string]$BuildRoot = 'D:/BuildCache/glory-cli-native-acceptance'
)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
$Driver = (Resolve-Path -LiteralPath $Driver).Path
$taskRoot = [IO.Path]::GetFullPath($BuildRoot)
if (-not $taskRoot.Replace('\','/').StartsWith('D:/BuildCache/', [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Native acceptance requires a D:/BuildCache root'
}
$taskRoot = Join-Path $taskRoot ([guid]::NewGuid().ToString('N'))
$fixture = Join-Path $taskRoot 'fixture'
$output = Join-Path $taskRoot 'build'
$runtime = (Resolve-Path -LiteralPath "$PSScriptRoot/../../../crates/glory").Path.Replace('\','/')
$saved = @{}
foreach ($name in @('CARGO_HOME','CARGO_TARGET_DIR','TEMP','TMP','PYTHONPYCACHEPREFIX')) {
    $saved[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
}
try {
    $env:CARGO_HOME = 'D:/BuildCache/glory-semantic-ui/cargo'
    $env:CARGO_TARGET_DIR = "$output/app-target"
    $env:TEMP = "$taskRoot/tmp"
    $env:TMP = $env:TEMP
    $env:PYTHONPYCACHEPREFIX = "$taskRoot/pycache"
    New-Item -ItemType Directory -Force "$fixture/src","$fixture/assets",$env:TEMP | Out-Null
    $hostLine = rustc -vV | Where-Object { $_.StartsWith('host: ') }
    if ($LASTEXITCODE -or -not $hostLine) { throw 'Missing Rust host' }
    $native = $hostLine.Substring(6)
    $foreign = if ($native -eq 'aarch64-apple-darwin') { 'x86_64-unknown-linux-gnu' } else { 'aarch64-apple-darwin' }
    @"
[package]
name = "cli-native-smoke"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
glory = { path = "$runtime" }
[package.metadata.glory.desktop]
target = "$native"
assets = ["assets"]
"@ | Set-Content -LiteralPath "$fixture/Cargo.toml" -Encoding utf8
    @'
fn main() {
    let _app = glory::App::memory();
    let executable = std::env::current_exe().unwrap();
    let asset = executable.parent().unwrap().join("assets/message.txt");
    assert_eq!(std::fs::read_to_string(asset).unwrap(), "relocated asset");
    println!("GLORY_NATIVE_BUNDLE_OK");
}
'@ | Set-Content -LiteralPath "$fixture/src/main.rs" -Encoding utf8
    [IO.File]::WriteAllText("$fixture/assets/message.txt", 'relocated asset')
    cargo generate-lockfile --offline --manifest-path "$fixture/Cargo.toml"
    if ($LASTEXITCODE) { throw 'Fixture lockfile failed' }
    $metadata = cargo metadata --offline --no-deps --format-version 1 --manifest-path "$fixture/Cargo.toml" | ConvertFrom-Json
    if ($LASTEXITCODE -or [IO.Path]::GetFullPath($metadata.target_directory) -ne [IO.Path]::GetFullPath($env:CARGO_TARGET_DIR)) { throw 'Unexpected Cargo target' }
    $lockHash = (Get-FileHash -LiteralPath "$fixture/Cargo.lock").Hash
    function Invoke-Driver([string[]]$Options, [int]$Expected = 0) {
        $messages = & $Driver @Options --manifest-path "$fixture/Cargo.toml" --build-root $output --offline 2>&1
        $code = $LASTEXITCODE
        $messages | Out-String | Add-Content -LiteralPath "$taskRoot/driver.log"
        if (($Expected -eq 0 -and $code -ne 0) -or ($Expected -ne 0 -and $code -eq 0)) {
            throw "Unexpected driver exit $code for $Options`: $messages"
        }
        return ($messages | Out-String)
    }
    $plan = Invoke-Driver @('print','--platform','desktop') | ConvertFrom-Json
    if ($plan.target -ne $native) { throw 'Configured target was not selected' }
    $override = Invoke-Driver @('print','--platform','desktop','--target',$foreign) | ConvertFrom-Json
    if ($override.target -ne $foreign -or $override.rust_host -ne $native) { throw 'CLI target did not override configuration' }
    Invoke-Driver @('doctor','--platform','desktop') | Write-Output
    $rejected = Invoke-Driver @('run','--platform','desktop','--target',$foreign) 1
    if (-not $rejected.Contains('cannot run target') -or (Test-Path -LiteralPath "$output/app-target/$foreign")) { throw 'Foreign target was built or run' }
    $mobile = Invoke-Driver @('build','--platform','desktop','--target','aarch64-linux-android') 1
    if (-not $mobile.Contains('dedicated application hosts')) { throw 'Mobile target was misreported as desktop' }
    $unknown = Invoke-Driver @('print','--platform','desktop','--target','imaginary-unknown-linux-gnu') 1
    if (-not $unknown.Contains('unknown built-in Rust target')) { throw 'Unknown target accepted' }
    $other = "$output/bundle/cli-native-smoke/desktop/$foreign"
    New-Item -ItemType Directory -Force $other | Out-Null
    'other architecture' | Set-Content -LiteralPath "$other/keep.txt"
    Invoke-Driver @('bundle','--platform','desktop') | Write-Output
    $bundle = "$output/bundle/cli-native-smoke/desktop/$native"
    $manifest = Get-Content -LiteralPath "$bundle/glory-build.json" -Raw | ConvertFrom-Json
    if ($manifest.target -ne $native -or $manifest.platform -ne 'desktop' -or -not $manifest.release -or $manifest.files.Count -ne 2) { throw 'Invalid bundle manifest' }
    foreach ($file in $manifest.files) {
        $path = Join-Path $bundle $file.path
        if ((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() -ne $file.sha256 -or (Get-Item -LiteralPath $path).Length -ne $file.bytes) { throw "Invalid file hash: $path" }
    }
    $relocated = "$taskRoot/relocated app"
    Copy-Item -LiteralPath $bundle -Destination $relocated -Recurse
    $message = & "$relocated/cli-native-smoke.exe"
    if ($LASTEXITCODE -or $message -ne 'GLORY_NATIVE_BUNDLE_OK') { throw 'Relocated bundle did not run with its assets' }
    if (-not (Test-Path -LiteralPath "$other/keep.txt")) { throw 'Other architecture bundle was replaced' }
    if ((Get-FileHash -LiteralPath "$fixture/Cargo.lock").Hash -ne $lockHash) { throw 'CLI changed the application lockfile' }
    @{
        host = $native; target = $manifest.target; configuredTarget = $true; targetOverride = $true
        foreignRunRejected = $true; mobileHostRequired = $true; unknownTargetRejected = $true
        relocatedBinaryAndAssets = $true; hashesVerified = $true; otherArchitecturePreserved = $true
        lockfilePreserved = $true; bundle = $bundle
    } | ConvertTo-Json | Set-Content -LiteralPath "$taskRoot/result.json"
    Write-Output "Native CLI acceptance passed: $taskRoot/result.json"
}
finally {
    foreach ($name in $saved.Keys) { [Environment]::SetEnvironmentVariable($name, $saved[$name], 'Process') }
}
