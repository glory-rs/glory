param([string]$BuildRoot='D:/BuildCache/glory-portfolio-20261025',[Parameter(Mandatory=$true)][string]$SourceRoot)
$ErrorActionPreference='Stop'
$BuildRoot=[IO.Path]::GetFullPath($BuildRoot); $SourceRoot=[IO.Path]::GetFullPath($SourceRoot)
foreach($taskPath in @($BuildRoot,$SourceRoot)){if(-not $taskPath.Replace('\','/').StartsWith('D:/BuildCache/',[StringComparison]::OrdinalIgnoreCase)){throw 'Use D:/BuildCache'}}
$saved=@{}; foreach($key in @('CARGO_HOME','CARGO_TARGET_DIR','TEMP','TMP','CARGO_BUILD_JOBS')){$saved[$key]=[Environment]::GetEnvironmentVariable($key,'Process')}
try {
 New-Item -ItemType Directory -Force -Path "$BuildRoot/tmp" | Out-Null
 $env:CARGO_HOME='D:/BuildCache/glory-semantic-ui/cargo';$env:TEMP="$BuildRoot/tmp";$env:TMP=$env:TEMP;$env:CARGO_BUILD_JOBS='2'
 $bindgen='D:/BuildCache/glory-framework-compare/tools/bin/wasm-bindgen.exe'
 if((& $bindgen --version).Trim() -ne 'wasm-bindgen 0.2.129'){throw 'Wrong bindgen'}
 foreach($variant in @('glory','dioxus-root','dioxus-row')){
  $app=if($variant -eq 'glory'){'glory'}else{'dioxus'}; $manifest="$SourceRoot/benchmarks/scenarios/$app/Cargo.toml"
  $flags=@();if($variant -eq 'dioxus-row'){$flags=@('--features','rows')}
  $env:CARGO_TARGET_DIR="$BuildRoot/$variant/target"
  $metadata=cargo metadata --locked --manifest-path $manifest --no-deps --format-version 1 @flags | ConvertFrom-Json
  if($LASTEXITCODE -ne 0 -or [IO.Path]::GetFullPath($metadata.target_directory) -ne [IO.Path]::GetFullPath($env:CARGO_TARGET_DIR)){throw 'Wrong target'}
  $metadata | ConvertTo-Json -Depth 100 | Set-Content "$BuildRoot/$variant-metadata-final.json"
  if(Test-Path "$env:CARGO_TARGET_DIR/wasm32-unknown-unknown/release/scenario-$app.wasm"){throw 'Use a fresh release target'}
  cargo clippy --locked --manifest-path $manifest --target wasm32-unknown-unknown --all-targets @flags -- -D warnings > "$BuildRoot/$variant-clippy.txt" 2>&1
  if($LASTEXITCODE -ne 0){throw "$variant clippy failed"}
  cargo build --locked --manifest-path $manifest --target wasm32-unknown-unknown --release @flags --message-format=json-render-diagnostics > "$BuildRoot/$variant-build.jsonl" 2> "$BuildRoot/$variant-build.txt"
  if($LASTEXITCODE -ne 0){throw "$variant build failed"}
  $site="$BuildRoot/site/$variant";New-Item -ItemType Directory -Force -Path $site | Out-Null
  & $bindgen "$env:CARGO_TARGET_DIR/wasm32-unknown-unknown/release/scenario-$app.wasm" --target web --out-dir $site --out-name app
  if($LASTEXITCODE -ne 0){throw 'bindgen failed'}
  Copy-Item -LiteralPath "$SourceRoot/benchmarks/scenarios/index.html","$SourceRoot/benchmarks/scenarios/style.css" -Destination $site
  Write-Output "Built $variant independently"
 }
} finally {foreach($key in $saved.Keys){[Environment]::SetEnvironmentVariable($key,$saved[$key],'Process')}}
