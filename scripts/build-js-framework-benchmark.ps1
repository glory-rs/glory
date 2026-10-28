param(
 [ValidateSet('glory','leptos','dioxus')][string[]]$Frameworks=@('glory','leptos','dioxus'),
 [string]$BuildRoot='D:/BuildCache/glory-display-completion-20261028/js-framework',
 [string]$BenchmarkRoot="$PSScriptRoot/../../js-framework-benchmark",
 [string]$Bindgen='D:/BuildCache/glory-v2-dioxus-20261028/tools/wasm-bindgen-0.2.129-x86_64-pc-windows-msvc/wasm-bindgen.exe'
)
$ErrorActionPreference='Stop'
$BuildRoot=[IO.Path]::GetFullPath($BuildRoot)
if(-not $BuildRoot.Replace('\','/').StartsWith('D:/BuildCache/',[StringComparison]::OrdinalIgnoreCase)){throw 'Use a D:/BuildCache build root'}
$BenchmarkRoot=[IO.Path]::GetFullPath($BenchmarkRoot)
$saved=@{}
foreach($key in @('CARGO_HOME','CARGO_TARGET_DIR','TEMP','TMP','CARGO_BUILD_JOBS')){$saved[$key]=[Environment]::GetEnvironmentVariable($key,'Process')}
try {
 $env:CARGO_HOME='D:/BuildCache/glory-semantic-ui/cargo';$env:TEMP="$BuildRoot/tmp";$env:TMP=$env:TEMP;$env:CARGO_BUILD_JOBS='2'
 New-Item -ItemType Directory -Force -Path $env:TEMP | Out-Null
 if((& $Bindgen --version).Trim() -ne 'wasm-bindgen 0.2.129'){throw 'Wrong bindgen CLI'}
 foreach($framework in $Frameworks){
  $source="$BenchmarkRoot/frameworks/keyed/$framework";$manifest="$source/Cargo.toml"
  $env:CARGO_TARGET_DIR="$BuildRoot/$framework/target"
  $metadata=cargo metadata --locked --manifest-path $manifest --no-deps --format-version 1 | ConvertFrom-Json
  if($LASTEXITCODE -ne 0 -or [IO.Path]::GetFullPath($metadata.target_directory) -ne [IO.Path]::GetFullPath($env:CARGO_TARGET_DIR)){throw 'Wrong target directory'}
  $metadata|ConvertTo-Json -Depth 100|Set-Content "$BuildRoot/$framework-metadata.json"
  cargo build --locked --manifest-path $manifest --release --target wasm32-unknown-unknown --message-format=json-render-diagnostics > "$BuildRoot/$framework-build.jsonl" 2> "$BuildRoot/$framework-build.txt"
  if($LASTEXITCODE -ne 0){Get-Content "$BuildRoot/$framework-build.txt" -Tail 35;throw "$framework build failed"}
  $artifact=Get-Content "$BuildRoot/$framework-build.jsonl"|ForEach-Object{$_|ConvertFrom-Json}|Where-Object{$_.reason -eq 'compiler-artifact' -and [IO.Path]::GetFullPath($_.manifest_path) -eq [IO.Path]::GetFullPath($manifest)}|Select-Object -Last 1
  $wasm=@($artifact.filenames|Where-Object{$_.EndsWith('.wasm')})
  if($wasm.Count -ne 1 -or -not [IO.Path]::GetFullPath($wasm[0]).StartsWith([IO.Path]::GetFullPath($env:CARGO_TARGET_DIR))){throw 'Unexpected WASM artifact'}
  $site="$source/bundled-dist";New-Item -ItemType Directory -Force -Path $site | Out-Null
  & $Bindgen $wasm[0] --target web --out-dir $site --out-name app
  if($LASTEXITCODE -ne 0){throw 'Bindgen failed'}
  $index='<!doctype html><html lang="en"><head><meta charset="utf-8"><title>'+ $framework +'</title><link href="/css/currentStyle.css" rel="stylesheet"></head><body><div id="main"></div><script type="module">import init from "./app.js";await init();</script></body></html>'
  [IO.File]::WriteAllText("$site/index.html",$index,[Text.UTF8Encoding]::new($false))
  if($framework -eq 'dioxus'){Copy-Item -LiteralPath "$site/index.html" -Destination "$source/index.html"}
  Write-Output "Built keyed/$framework from source: $site"
 }
} finally {
 foreach($key in $saved.Keys){
  if([string]::IsNullOrEmpty($saved[$key])){Remove-Item -LiteralPath "Env:$key" -ErrorAction SilentlyContinue}
  else{[Environment]::SetEnvironmentVariable($key,$saved[$key],'Process')}
 }
}
