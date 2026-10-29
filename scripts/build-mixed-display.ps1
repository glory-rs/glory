param(
 [string]$BuildRoot='D:/BuildCache/glory-semantic-ui',
 [string]$Bindgen='D:/BuildCache/glory-v2-dioxus-20261028/tools/wasm-bindgen-0.2.129-x86_64-pc-windows-msvc/wasm-bindgen.exe',
 [switch]$Verify
)
$ErrorActionPreference='Stop'
$BuildRoot=[IO.Path]::GetFullPath($BuildRoot)
if(-not $BuildRoot.Replace('\','/').StartsWith('D:/BuildCache/',[StringComparison]::OrdinalIgnoreCase)){throw 'BuildRoot must be on D:/BuildCache'}
$saved=@{}
foreach($key in @('CARGO_HOME','CARGO_TARGET_DIR','TEMP','TMP','CARGO_BUILD_JOBS','CARGO_INCREMENTAL','CARGO_PROFILE_DEV_DEBUG','GLORY_MIXED_SITE')){$saved[$key]=[Environment]::GetEnvironmentVariable($key,'Process')}
try {
 $env:CARGO_HOME='D:/BuildCache/glory-semantic-ui/cargo';$env:CARGO_TARGET_DIR="$BuildRoot/target"
 $env:TEMP="$BuildRoot/tmp";$env:TMP=$env:TEMP;$env:CARGO_BUILD_JOBS='2';$env:CARGO_INCREMENTAL='0';$env:CARGO_PROFILE_DEV_DEBUG='0'
 $site="$BuildRoot/mixed-site";New-Item -ItemType Directory -Force -Path $site,$env:TEMP|Out-Null
 $manifest=[IO.Path]::GetFullPath("$PSScriptRoot/../examples/display-web/Cargo.toml")
 $metadata=cargo metadata --manifest-path $manifest --no-deps --format-version 1 | ConvertFrom-Json
 if($LASTEXITCODE -ne 0 -or [IO.Path]::GetFullPath($metadata.target_directory) -ne [IO.Path]::GetFullPath($env:CARGO_TARGET_DIR)){throw 'Unexpected target directory'}
 cargo run --locked --manifest-path $manifest --bin mixed-ssr > "$site/snapshot.html"
 if($LASTEXITCODE -ne 0){throw 'SSR fixture failed'}
 cargo build --locked --manifest-path $manifest --bin mixed-web --target wasm32-unknown-unknown
 if($LASTEXITCODE -ne 0){throw 'WASM fixture failed'}
 if((& $Bindgen --version).Trim() -ne 'wasm-bindgen 0.2.129'){throw 'Expected pinned bindgen 0.2.129'}
 & $Bindgen "$env:CARGO_TARGET_DIR/wasm32-unknown-unknown/debug/mixed-web.wasm" --target web --out-dir $site --out-name app
 if($LASTEXITCODE -ne 0){throw 'Bindgen failed'}
 if($Verify){$env:GLORY_MIXED_SITE=$site;node "$PSScriptRoot/../examples/display-web/mixed-acceptance.cjs";if($LASTEXITCODE -ne 0){throw 'Mixed browser acceptance failed'}}
 Write-Output "Mixed SSR/browser fixture: $site"
} finally {foreach($key in $saved.Keys){[Environment]::SetEnvironmentVariable($key,$saved[$key],'Process')}}
