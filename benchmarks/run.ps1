# Build a frozen release snapshot and serve it from verified D: storage.
#   ./run.ps1 glory | leptos | dioxus
param(
  [Parameter(Mandatory = $true)]
  [ValidateSet('glory', 'leptos', 'dioxus')]
  [string]$App
)

$ErrorActionPreference='Stop'
$taskBuild='D:/BuildCache/glory-framework-compare'
& "$PSScriptRoot/build.ps1" -Apps $App -BuildRoot $taskBuild
Write-Output "Preview: http://127.0.0.1:8790/$App/"
Write-Output 'For a measured three-framework run, use benchmarks/compare.ps1.'
$savedCache=$env:PYTHONPYCACHEPREFIX
try{
 $env:PYTHONPYCACHEPREFIX="$taskBuild/pycache"
 python -m http.server 8790 --bind 127.0.0.1 --directory "$taskBuild/site"
}finally{$env:PYTHONPYCACHEPREFIX=$savedCache}
