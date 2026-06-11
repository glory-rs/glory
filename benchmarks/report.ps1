# Generate a local benchmark report bundle.
# The report includes Criterion logs for core benches plus wasm size JSON.
param(
  [switch]$SkipCargoBench,
  [switch]$SkipWasmBuild,
  [switch]$SkipPlaywright,
  [string]$OutDir = "target/benchmark-report"
)

$ErrorActionPreference = 'Stop'
$Root = Resolve-Path (Join-Path $PSScriptRoot '..')
$ReportDir = Join-Path $Root $OutDir
New-Item -ItemType Directory -Force -Path $ReportDir | Out-Null

function Invoke-LoggedCargo {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Name,
    [Parameter(Mandatory = $true)]
    [string[]]$Args
  )

  $log = Join-Path $ReportDir "$Name.log"
  & cargo @Args *>&1 | Tee-Object -FilePath $log
  if ($LASTEXITCODE -ne 0) {
    throw "cargo $($Args -join ' ') failed; see $log"
  }
  return $log
}

function Invoke-LoggedProcess {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Name,
    [Parameter(Mandatory = $true)]
    [string]$FilePath,
    [Parameter(Mandatory = $true)]
    [string[]]$Args
  )

  $log = Join-Path $ReportDir "$Name.log"
  & $FilePath @Args *>&1 | Tee-Object -FilePath $log
  if ($LASTEXITCODE -ne 0) {
    throw "$FilePath $($Args -join ' ') failed; see $log"
  }
  return $log
}

$logs = @()
if (-not $SkipCargoBench) {
  $logs += Invoke-LoggedCargo 'command-wire' @('bench', '-p', 'glory-core', '--bench', 'command_wire', '--', '--save-baseline', 'glory-local')
  $logs += Invoke-LoggedCargo 'each-reorder' @('bench', '-p', 'glory-core', '--features', 'web-ssr', '--bench', 'each_reorder', '--', '--save-baseline', 'glory-local')
  $logs += Invoke-LoggedCargo 'scheduler' @('bench', '-p', 'glory-core', '--features', 'web-ssr', '--bench', 'scheduler', '--', '--save-baseline', 'glory-local')
  $logs += Invoke-LoggedCargo 'ssr-stream' @('bench', '-p', 'glory-core', '--features', 'web-ssr', '--bench', 'ssr_stream', '--', '--save-baseline', 'glory-local')
}

$wasmJson = Join-Path $ReportDir 'wasm-size.json'
if ($SkipWasmBuild) {
  $wasmOutput = & (Join-Path $PSScriptRoot 'wasm-size.ps1') -Json -SkipBuild
} else {
  $wasmOutput = & (Join-Path $PSScriptRoot 'wasm-size.ps1') -Json
}
$wasmOutput | Set-Content -Path $wasmJson

$playwrightStatusPath = Join-Path $ReportDir 'playwright-status.json'
$playwrightStatus = [PSCustomObject]@{
  Status = 'skipped'
  Reason = ''
  Log = $null
  Json = $null
  Projects = @('web-csr-counter', 'web-csr-routing', 'ssr-hydration', 'fullstack-serverfn', 'hot-reload')
  UrlEnvironment = [PSCustomObject]@{
    GLORY_COUNTER_URL = $env:GLORY_COUNTER_URL
    GLORY_ROUTER_URL = $env:GLORY_ROUTER_URL
    GLORY_SSR_URL = $env:GLORY_SSR_URL
    GLORY_FULLSTACK_URL = $env:GLORY_FULLSTACK_URL
    GLORY_HOT_RELOAD_URL = $env:GLORY_HOT_RELOAD_URL
  }
}
if ($SkipPlaywright) {
  $playwrightStatus.Reason = 'SkipPlaywright was set'
} elseif (-not (Get-Command npm -ErrorAction SilentlyContinue)) {
  $playwrightStatus.Reason = 'npm was not found on PATH'
} elseif (-not (Test-Path (Join-Path $Root 'tests/playwright/package.json'))) {
  $playwrightStatus.Reason = 'tests/playwright/package.json was not found'
} else {
  $playwrightJson = Join-Path $ReportDir 'playwright.json'
  $previousJsonOutput = $env:PLAYWRIGHT_JSON_OUTPUT_NAME
  $env:PLAYWRIGHT_JSON_OUTPUT_NAME = $playwrightJson
  try {
    $playwrightLog = Invoke-LoggedProcess 'playwright-e2e' 'npm' @('--prefix', (Join-Path $Root 'tests/playwright'), 'run', 'test', '--', '--reporter=json')
    $playwrightStatus.Status = 'completed'
    $playwrightStatus.Reason = ''
    $playwrightStatus.Log = $playwrightLog
    $playwrightStatus.Json = $playwrightJson
  } finally {
    $env:PLAYWRIGHT_JSON_OUTPUT_NAME = $previousJsonOutput
  }
}
$playwrightStatus | ConvertTo-Json -Depth 5 | Set-Content -Path $playwrightStatusPath

$summary = Join-Path $ReportDir 'summary.md'
$lines = @(
  '# Glory Benchmark Report',
  '',
  "Generated: $(Get-Date -Format s)",
  '',
  '## Artifacts',
  '',
  "- Wasm size JSON: $wasmJson",
  "- Playwright CSR/hydration status JSON: $playwrightStatusPath"
)
foreach ($log in $logs) {
  $lines += "- Criterion log: $log"
}
if ($playwrightStatus.Log) {
  $lines += "- Playwright CSR/hydration log: $($playwrightStatus.Log)"
}
$lines += ''
$lines += 'Criterion HTML output is under target/criterion/ when cargo benches run.'
$lines | Set-Content -Path $summary

Write-Output $summary
