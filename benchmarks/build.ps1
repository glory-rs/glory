param(
 [ValidateSet('glory','leptos','dioxus')][string[]]$Apps=@('glory','leptos','dioxus'),
 [string]$BuildRoot='D:/BuildCache/glory-framework-compare',
 [string]$SourceRoot='',
 [string]$Bindgen='',
 [switch]$SkipBuild,
 [switch]$Handwritten,
 [switch]$Probes
)
$ErrorActionPreference='Stop'
$BuildRoot=[IO.Path]::GetFullPath($BuildRoot)
if(-not $BuildRoot.Replace('\','/').StartsWith('D:/BuildCache/',[StringComparison]::OrdinalIgnoreCase)){throw 'BuildRoot must be on D:/BuildCache'}
New-Item -ItemType Directory -Force -Path "$BuildRoot/tmp","$BuildRoot/pycache","$BuildRoot/tools/bin" | Out-Null
$saved=@{}
foreach($key in @('CARGO_HOME','CARGO_TARGET_DIR','TEMP','TMP','PYTHONPYCACHEPREFIX','CARGO_BUILD_JOBS')){$saved[$key]=[Environment]::GetEnvironmentVariable($key,'Process')}
try {
 $env:CARGO_HOME='D:/BuildCache/glory-semantic-ui/cargo'
 $env:TEMP="$BuildRoot/tmp"; $env:TMP=$env:TEMP
 $env:PYTHONPYCACHEPREFIX="$BuildRoot/pycache"; $env:CARGO_BUILD_JOBS='2'
 if(-not $SourceRoot){
  $SourceRoot="$BuildRoot/source-$(Get-Date -Format yyyyMMdd-HHmmss-fff)"
  python "$PSScriptRoot/snapshot.py" --out $SourceRoot
  if($LASTEXITCODE -ne 0){throw 'snapshot failed'}
 }
 $SourceRoot=[IO.Path]::GetFullPath($SourceRoot)
 if(-not $SourceRoot.Replace('\','/').StartsWith('D:/BuildCache/',[StringComparison]::OrdinalIgnoreCase)){throw 'Source snapshot must be on D:/BuildCache'}
 if(-not $Bindgen){$Bindgen="$BuildRoot/tools/bin/wasm-bindgen.exe"}
 $version=& $Bindgen --version
 if($LASTEXITCODE -ne 0 -or $version.Trim() -ne 'wasm-bindgen 0.2.129'){throw 'Use wasm-bindgen-cli 0.2.129 installed under D:/BuildCache'}
 foreach($app in $Apps){
  $featureArgs=@()
  if($Handwritten){if($app -ne 'glory'){throw 'Handwritten applies only to glory'}; $featureArgs=@('--features','handwritten')}
  if($Probes){if($app -ne 'glory'){throw 'Probes apply only to glory'}; $featureArgs+=@('--features','acceptance')}
  $manifest="$SourceRoot/benchmarks/$app/Cargo.toml"
  $env:CARGO_TARGET_DIR="$BuildRoot/$app/target"
  $metadata=cargo metadata --manifest-path $manifest --no-deps --format-version 1 | ConvertFrom-Json
  if($LASTEXITCODE -ne 0 -or [IO.Path]::GetFullPath($metadata.target_directory) -ne [IO.Path]::GetFullPath($env:CARGO_TARGET_DIR)){throw "Wrong target for $app"}
  $metadata | ConvertTo-Json -Depth 100 | Set-Content "$BuildRoot/$app-metadata-final.json"
  if(-not $SkipBuild){
   cargo clippy --locked --manifest-path $manifest @featureArgs --target wasm32-unknown-unknown --all-targets -- -D warnings > "$BuildRoot/$app-clippy.txt" 2>&1
   if($LASTEXITCODE -ne 0){throw "$app clippy failed"}
   cargo build --locked --manifest-path $manifest @featureArgs --release --target wasm32-unknown-unknown --message-format=json-render-diagnostics > "$BuildRoot/$app-build.jsonl" 2> "$BuildRoot/$app-build.txt"
   if($LASTEXITCODE -ne 0){throw "$app release build failed"}
  }
  $site="$BuildRoot/site/$app"
  New-Item -ItemType Directory -Force -Path $site | Out-Null
  $artifact=Get-Content "$BuildRoot/$app-build.jsonl" | ForEach-Object {$_ | ConvertFrom-Json} | Where-Object {$_.reason -eq 'compiler-artifact' -and $_.target.name -eq "bench-$app" -and $_.target.kind -contains 'bin'} | Select-Object -Last 1
  if(-not $artifact -or [IO.Path]::GetFullPath($artifact.manifest_path) -ne [IO.Path]::GetFullPath($manifest)){throw 'Missing/mismatched application compiler artifact'}
  $wasm=@($artifact.filenames | Where-Object {$_.EndsWith('.wasm')})
  if($wasm.Count -ne 1 -or -not [IO.Path]::GetFullPath($wasm[0]).StartsWith([IO.Path]::GetFullPath($env:CARGO_TARGET_DIR),[StringComparison]::OrdinalIgnoreCase)){throw 'Unexpected WASM output'}
  & $Bindgen $wasm[0] --target web --out-dir $site --out-name app
  if($LASTEXITCODE -ne 0){throw "$app bindgen failed"}
  foreach($name in @('bench-harness.js','semantic.css')){Copy-Item -LiteralPath "$SourceRoot/benchmarks/$name" -Destination "$site/$name"}
  $index=Get-Content "$SourceRoot/benchmarks/$app/index.html" -Raw
  $index=$index -replace '<link data-trunk[^>]+>',''
  $bootstrap='<script type="module">import init,* as app from "./app.js";await init();if(app.stats)window.__BENCH_STATS__=()=>JSON.parse(app.stats());window.__BENCH_READY__=true;</script>'
  $index=$index.Replace('</body>',"$bootstrap</body>")
  [IO.File]::WriteAllText("$site/index.html",$index,[Text.UTF8Encoding]::new($false))
  Write-Output "Built $app independently from $SourceRoot; target=$env:CARGO_TARGET_DIR"
 }
} finally {
 foreach($key in $saved.Keys){
  if($null -eq $saved[$key]){Remove-Item -LiteralPath "Env:$key" -ErrorAction SilentlyContinue}
  else{[Environment]::SetEnvironmentVariable($key,$saved[$key],'Process')}
 }
}
