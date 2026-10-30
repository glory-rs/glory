param(
 [string]$BuildRoot='D:/BuildCache/glory-semantic-ui',
 [string]$Bindgen='D:/BuildCache/glory-v2-dioxus-20261028/tools/wasm-bindgen-0.2.130-x86_64-pc-windows-msvc/wasm-bindgen.exe',
 [switch]$Verify
)
$ErrorActionPreference='Stop'
$BuildRoot=[IO.Path]::GetFullPath($BuildRoot)
if(-not $BuildRoot.Replace('\','/').StartsWith('D:/BuildCache/',[StringComparison]::OrdinalIgnoreCase)){throw 'BuildRoot must be on D:/BuildCache'}
$saved=@{}
foreach($key in @('CARGO_HOME','CARGO_TARGET_DIR','TEMP','TMP','CARGO_BUILD_JOBS','CARGO_INCREMENTAL','CARGO_PROFILE_DEV_DEBUG','GLORY_MIXED_STREAM_SITE','GLORY_MIXED_STREAM_BIN')){$saved[$key]=[Environment]::GetEnvironmentVariable($key,'Process')}
try {
 $env:CARGO_HOME='D:/BuildCache/glory-semantic-ui/cargo';$env:CARGO_TARGET_DIR="$BuildRoot/target"
 $env:TEMP="$BuildRoot/tmp";$env:TMP=$env:TEMP;$env:CARGO_BUILD_JOBS='2';$env:CARGO_INCREMENTAL='0';$env:CARGO_PROFILE_DEV_DEBUG='0'
 $site="$BuildRoot/mixed-stream-site";New-Item -ItemType Directory -Force -Path $site,$env:TEMP|Out-Null
 $manifest=[IO.Path]::GetFullPath("$PSScriptRoot/../examples/display-web/Cargo.toml")
 $metadata=cargo metadata --manifest-path $manifest --no-deps --format-version 1 | ConvertFrom-Json
 if($LASTEXITCODE -ne 0 -or [IO.Path]::GetFullPath($metadata.target_directory) -ne [IO.Path]::GetFullPath($env:CARGO_TARGET_DIR)){throw 'Unexpected target directory'}
 cargo build --locked --manifest-path $manifest --bin mixed-stream-ssr
 if($LASTEXITCODE -ne 0){throw 'Stream server build failed'}
 cargo build --locked --manifest-path $manifest --bin mixed-stream-web --target wasm32-unknown-unknown
 if($LASTEXITCODE -ne 0){throw 'Stream hydration build failed'}
 if((& $Bindgen --version).Trim() -ne 'wasm-bindgen 0.2.130'){throw 'Expected pinned bindgen 0.2.130'}
 & $Bindgen "$env:CARGO_TARGET_DIR/wasm32-unknown-unknown/debug/mixed-stream-web.wasm" --target web --out-dir $site --out-name app
 if($LASTEXITCODE -ne 0){throw 'Bindgen failed'}
 if($Verify){$env:GLORY_MIXED_STREAM_SITE=$site;$env:GLORY_MIXED_STREAM_BIN="$env:CARGO_TARGET_DIR/debug/mixed-stream-ssr.exe";node "$PSScriptRoot/../examples/display-web/mixed-stream-acceptance.cjs";if($LASTEXITCODE -ne 0){throw 'Mixed stream acceptance failed'}}
 Write-Output "Mixed stream fixture: $site"
} finally {foreach($key in $saved.Keys){[Environment]::SetEnvironmentVariable($key,$saved[$key],'Process')}}
