param(
 [string]$BuildRoot='D:/BuildCache/glory-dsl-20261028',
 [string]$Bindgen='D:/BuildCache/glory-v2-dioxus-20261028/tools/wasm-bindgen-0.2.129-x86_64-pc-windows-msvc/wasm-bindgen.exe',
 [switch]$Hand,
 [ValidateSet('display-web','counter')][string]$Bin='display-web'
)
$ErrorActionPreference='Stop'
$BuildRoot=[IO.Path]::GetFullPath($BuildRoot)
if(-not $BuildRoot.Replace('\','/').StartsWith('D:/BuildCache/',[StringComparison]::OrdinalIgnoreCase)){throw 'BuildRoot must be on D:/BuildCache'}
if($Hand -and $Bin -ne 'display-web'){throw 'Hand selects the acceptance comparison only'}
$variant=if($Bin -eq 'counter'){'counter'}elseif($Hand){'hand'}else{'dsl'}
$saved=@{}
foreach($key in @('CARGO_HOME','CARGO_TARGET_DIR','TEMP','TMP','CARGO_BUILD_JOBS')){$saved[$key]=[Environment]::GetEnvironmentVariable($key,'Process')}
try {
 $env:CARGO_HOME='D:/BuildCache/glory-semantic-ui/cargo';$env:CARGO_TARGET_DIR="$BuildRoot/$variant/target"
 $env:TEMP="$BuildRoot/tmp";$env:TMP=$env:TEMP;$env:CARGO_BUILD_JOBS='2'
 New-Item -ItemType Directory -Force -Path "$BuildRoot/$variant",$env:TEMP | Out-Null
 $manifest=[IO.Path]::GetFullPath("$PSScriptRoot/../examples/display-web/Cargo.toml")
 $flags=@();if($Hand){$flags=@('--features','hand')}
 $metadata=cargo metadata --manifest-path $manifest --no-deps --format-version 1 | ConvertFrom-Json
 if($LASTEXITCODE -ne 0 -or [IO.Path]::GetFullPath($metadata.target_directory) -ne [IO.Path]::GetFullPath($env:CARGO_TARGET_DIR)){throw 'Unexpected target directory'}
 $metadata|ConvertTo-Json -Depth 100|Set-Content "$BuildRoot/$variant/metadata.json"
 cargo clippy --locked --manifest-path $manifest @flags --all-targets --target wasm32-unknown-unknown -- -D warnings > "$BuildRoot/$variant/clippy.txt" 2>&1
 if($LASTEXITCODE -ne 0){Get-Content "$BuildRoot/$variant/clippy.txt" -Tail 45;throw 'Display fixture clippy failed'}
 cargo build --locked --manifest-path $manifest --bin $Bin @flags --release --target wasm32-unknown-unknown --message-format=json-render-diagnostics > "$BuildRoot/$variant/build.jsonl" 2> "$BuildRoot/$variant/build.txt"
 if($LASTEXITCODE -ne 0){Get-Content "$BuildRoot/$variant/build.txt" -Tail 35;throw 'Display fixture build failed'}
 $artifact=Get-Content "$BuildRoot/$variant/build.jsonl"|ForEach-Object{$_|ConvertFrom-Json}|Where-Object{$_.reason -eq 'compiler-artifact' -and $_.target.name -eq $Bin -and $_.target.kind -contains 'bin'}|Select-Object -Last 1
 if(-not $artifact -or [IO.Path]::GetFullPath($artifact.manifest_path) -ne $manifest){throw 'Wrong compiler artifact'}
 $wasm=@($artifact.filenames|Where-Object{$_.EndsWith('.wasm')})
 if($wasm.Count -ne 1 -or -not [IO.Path]::GetFullPath($wasm[0]).StartsWith([IO.Path]::GetFullPath($env:CARGO_TARGET_DIR))){throw 'Wrong WASM path'}
 $site="$BuildRoot/$variant/site";New-Item -ItemType Directory -Force -Path $site|Out-Null
 if((& $Bindgen --version).Trim() -ne 'wasm-bindgen 0.2.129'){throw 'Expected pinned bindgen 0.2.129'}
 & $Bindgen $wasm[0] --target web --out-dir $site --out-name app
 if($LASTEXITCODE -ne 0){throw 'Bindgen failed'}
 Copy-Item -LiteralPath "$PSScriptRoot/../examples/display-web/index.html" -Destination "$site/index.html"
 Write-Output "Built $variant display fixture: $site"
} finally {foreach($key in $saved.Keys){[Environment]::SetEnvironmentVariable($key,$saved[$key],'Process')}}
