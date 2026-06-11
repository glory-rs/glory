# Prepare and optionally run Glory/Dioxus adapters inside the official
# krausest/js-framework-benchmark Chrome tracing workflow.
param(
  [string]$BenchmarkRepo = $env:JS_FRAMEWORK_BENCHMARK_REPO,
  [ValidateSet('glory', 'dioxus', 'leptos')]
  [string[]]$Apps = @('glory', 'dioxus'),
  [switch]$NoClone,
  [switch]$SkipInstall,
  [switch]$SkipBuild,
  [switch]$SkipBench,
  [switch]$SkipResults,
  [string]$ChromeBinary = '',
  [string[]]$Benchmarks = @(),
  [int]$Count = 0,
  [switch]$Headless,
  [switch]$NoThrottling,
  [string]$OutDir = 'target/benchmark-report/official-js-framework'
)

$ErrorActionPreference = 'Stop'
$Root = Resolve-Path (Join-Path $PSScriptRoot '..')
if (-not $BenchmarkRepo) {
  $BenchmarkRepo = Join-Path $Root 'target/external/js-framework-benchmark'
}
$ReportDir = Join-Path $Root $OutDir
New-Item -ItemType Directory -Force -Path $ReportDir | Out-Null

$steps = New-Object System.Collections.Generic.List[object]

function Add-Step([string]$name, [string]$status, [string]$detail) {
  $steps.Add([PSCustomObject]@{
    Name = $name
    Status = $status
    Detail = $detail
  }) | Out-Null
}

function Invoke-LoggedProcess {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Name,
    [Parameter(Mandatory = $true)]
    [string]$WorkingDirectory,
    [Parameter(Mandatory = $true)]
    [string]$Program,
    [Parameter(Mandatory = $true)]
    [string[]]$ProgramArgs
  )

  $log = Join-Path $ReportDir "$Name.log"
  $resolvedProgram = Resolve-Program $Program
  Push-Location $WorkingDirectory
  try {
    & $resolvedProgram @ProgramArgs *>&1 | Tee-Object -FilePath $log
    $exitCode = $LASTEXITCODE
  } finally {
    Pop-Location
  }
  if ($exitCode -ne 0) {
    throw "$Program $($ProgramArgs -join ' ') failed; see $log"
  }
  Add-Step $Name 'completed' $log
  return $log
}

function To-ForwardPath([string]$path) {
  (Resolve-Path $path).Path.Replace('\', '/')
}

function Resolve-Program([string]$program) {
  $candidates = if ($IsWindows) {
    @("$program.cmd", "$program.exe", $program)
  } else {
    @($program)
  }
  foreach ($candidate in $candidates) {
    $command = Get-Command $candidate -ErrorAction SilentlyContinue
    if ($command -and $command.Source) {
      return $command.Source
    }
  }
  return $program
}

function Stop-OfficialServer {
  param(
    [object]$Process,
    [string]$Reason
  )

  $stopped = @()
  if ($Process -and -not $Process.HasExited) {
    if ($IsWindows) {
      & taskkill.exe /PID $Process.Id /T /F | Out-Null
    } else {
      Stop-Process -Id $Process.Id -Force
    }
    $stopped += "pid=$($Process.Id)"
  }

  if ($IsWindows) {
    $owners = Get-NetTCPConnection -LocalPort 8080 -ErrorAction SilentlyContinue |
      Select-Object -ExpandProperty OwningProcess -Unique
    foreach ($owner in $owners) {
      $proc = Get-CimInstance Win32_Process -Filter "ProcessId=$owner" -ErrorAction SilentlyContinue
      if (-not $proc) {
        continue
      }
      $cmd = [string]$proc.CommandLine
      if ($cmd.Contains($BenchmarkRepo) -or ($cmd.Contains('js-framework-benchmark') -and $cmd.Contains('server'))) {
        & taskkill.exe /PID $owner /T /F | Out-Null
        $stopped += "port8080-pid=$owner"
      }
    }
  }

  if ($stopped.Count -gt 0) {
    Add-Step 'server' 'stopped' "$Reason; $($stopped -join ', ')"
  }
}

function Adapter-Name([string]$app) {
  switch ($app) {
    'glory' { 'glory-rs' }
    'dioxus' { 'dioxus-rs' }
    'leptos' { 'leptos-rs' }
  }
}

function Adapter-Title([string]$app) {
  switch ($app) {
    'glory' { 'Glory Rust' }
    'dioxus' { 'Dioxus Rust' }
    'leptos' { 'Leptos Rust' }
  }
}

function Write-AdapterPackage([string]$app, [string]$dest) {
  $name = Adapter-Name $app
  $title = Adapter-Title $app
  $homeUrl = switch ($app) {
    'glory' { 'https://github.com/glory-rs/glory' }
    'dioxus' { 'https://github.com/DioxusLabs/dioxus' }
    'leptos' { 'https://github.com/leptos-rs/leptos' }
  }
  $version = switch ($app) {
    'glory' { '0.0.0-local' }
    'dioxus' { '0.7' }
    'leptos' { '0.8' }
  }
  $publicUrl = "/frameworks/keyed/$name/dist/"
  $packageName = "js-framework-benchmark-$name"
  $package = [ordered]@{
    name = $packageName
    version = '0.0.0'
    private = $true
    description = "$title keyed benchmark adapter"
    scripts = [ordered]@{
      dev = 'trunk serve --release'
      'build-prod' = "trunk build --release --public-url $publicUrl"
    }
    'js-framework-benchmark' = [ordered]@{
      frameworkVersion = $version
      frameworkHomeURL = $homeUrl
      language = 'Rust'
      customURL = '/dist/'
    }
    license = 'Apache-2.0'
  }
  $package | ConvertTo-Json -Depth 5 | Set-Content -Path (Join-Path $dest 'package.json') -Encoding utf8

  $lock = [ordered]@{
    name = $packageName
    version = '0.0.0'
    lockfileVersion = 3
    requires = $true
    packages = [ordered]@{
      '' = [ordered]@{
        name = $packageName
        version = '0.0.0'
        license = 'Apache-2.0'
      }
    }
  }
  $lock | ConvertTo-Json -Depth 5 | Set-Content -Path (Join-Path $dest 'package-lock.json') -Encoding utf8
}

function Convert-IndexForOfficialRunner([string]$path, [string]$app) {
  $title = Adapter-Title $app
  $html = Get-Content $path -Raw
  $html = $html -replace '<title>.*?</title>', "<title>$title</title>"
  $html = $html -replace "(?m)^\s*<link data-trunk rel=`"copy-file`".*\r?\n", ''
  $html = $html -replace "(?m)^\s*<script type=`"module`" src=`"/bench-harness\.js`"></script>\r?\n", ''
  $html | Set-Content -Path $path -Encoding utf8
}

function Patch-CargoToml([string]$app, [string]$dest) {
  $cargoToml = Join-Path $dest 'Cargo.toml'
  $content = Get-Content $cargoToml -Raw
  if ($app -eq 'glory') {
    $gloryPath = To-ForwardPath (Join-Path $Root 'crates/glory')
    $content = $content.Replace('path = "../../crates/glory"', "path = `"$gloryPath`"")
  }
  if ($content -notmatch '(?m)^\[workspace\]\s*$') {
    $content = $content.TrimEnd() + "`r`n`r`n[workspace]`r`n"
  }
  $content | Set-Content -Path $cargoToml -Encoding utf8
}

function Write-Status {
  param([string]$status)

  $statusPath = Join-Path $ReportDir 'official-js-framework-status.json'
  [PSCustomObject]@{
    Status = $status
    Generated = (Get-Date -Format s)
    BenchmarkRepo = (Resolve-Path $BenchmarkRepo -ErrorAction SilentlyContinue).Path
    Apps = $Apps
    FrameworkArgs = $frameworkArgs
    ChromeBinary = $ChromeBinary
    Benchmarks = $Benchmarks
    Count = $Count
    Headless = [bool]$Headless
    NoThrottling = [bool]$NoThrottling
    Steps = $steps
  } | ConvertTo-Json -Depth 6 | Set-Content -Path $statusPath -Encoding utf8
  return $statusPath
}

if (-not (Test-Path $BenchmarkRepo)) {
  if ($NoClone) {
    Add-Step 'clone' 'skipped' "missing benchmark repo: $BenchmarkRepo"
    $statusPath = Write-Status 'blocked'
    throw "Official js-framework-benchmark repo missing: $BenchmarkRepo; see $statusPath"
  }
  $parent = Split-Path -Parent $BenchmarkRepo
  New-Item -ItemType Directory -Force -Path $parent | Out-Null
  Invoke-LoggedProcess -Name 'official-clone' -WorkingDirectory $parent -Program 'git' -ProgramArgs @(
    'clone',
    '--depth',
    '1',
    'https://github.com/krausest/js-framework-benchmark.git',
    $BenchmarkRepo
  ) | Out-Null
}

if (-not (Test-Path (Join-Path $BenchmarkRepo 'package.json'))) {
  Add-Step 'repo-check' 'failed' 'package.json not found'
  $statusPath = Write-Status 'failed'
  throw "Not a js-framework-benchmark checkout: $BenchmarkRepo; see $statusPath"
}
Add-Step 'repo-check' 'completed' $BenchmarkRepo

$frameworkRoot = Join-Path $BenchmarkRepo 'frameworks/keyed'
New-Item -ItemType Directory -Force -Path $frameworkRoot | Out-Null

$frameworkArgs = @()
foreach ($app in $Apps) {
  $name = Adapter-Name $app
  $source = Join-Path $Root "benchmarks/$app"
  $dest = Join-Path $frameworkRoot $name
  if (-not (Test-Path $source)) {
    Add-Step "adapter-$app" 'failed' "missing local benchmark app: $source"
    $statusPath = Write-Status 'failed'
    throw "Missing local benchmark app: $source; see $statusPath"
  }
  if (Test-Path $dest) {
    Remove-Item -LiteralPath $dest -Recurse -Force
  }
  New-Item -ItemType Directory -Force -Path $dest | Out-Null
  Copy-Item -Path (Join-Path $source '*') -Destination $dest -Recurse -Force
  Convert-IndexForOfficialRunner (Join-Path $dest 'index.html') $app
  Patch-CargoToml $app $dest
  Write-AdapterPackage $app $dest
  $frameworkArgs += "keyed/$name"
  Add-Step "adapter-$app" 'completed' $dest
}

if (-not $SkipInstall) {
  Invoke-LoggedProcess -Name 'official-npm-ci' -WorkingDirectory $BenchmarkRepo -Program 'npm' -ProgramArgs @('ci') | Out-Null
  Invoke-LoggedProcess -Name 'official-install-local' -WorkingDirectory $BenchmarkRepo -Program 'npm' -ProgramArgs @('run', 'install-local') | Out-Null
} else {
  Add-Step 'install' 'skipped' 'SkipInstall was set'
}

if (-not $SkipBuild) {
  foreach ($framework in $frameworkArgs) {
    $dest = Join-Path $BenchmarkRepo "frameworks/$framework"
    $logName = "official-build-$($framework.Replace('/', '-'))"
    Invoke-LoggedProcess -Name $logName -WorkingDirectory $dest -Program 'npm' -ProgramArgs @('run', 'build-prod') | Out-Null
  }
} else {
  Add-Step 'build' 'skipped' 'SkipBuild was set'
}

$serverProcess = $null
if (-not $SkipBench) {
  if (-not $ChromeBinary) {
    $chromeCandidates = @(
      'C:\Program Files\Google\Chrome\Application\chrome.exe',
      'C:\Program Files (x86)\Google\Chrome\Application\chrome.exe',
      'C:\Program Files\Microsoft\Edge\Application\msedge.exe',
      'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'
    )
    $ChromeBinary = ($chromeCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1)
  }
  if (-not $ChromeBinary) {
    Add-Step 'bench' 'blocked' 'No Chrome/Edge binary found; pass -ChromeBinary.'
    $statusPath = Write-Status 'blocked'
    throw "No Chrome/Edge binary found for official benchmark; see $statusPath"
  }

  $serverOut = Join-Path $ReportDir 'official-server.out.log'
  $serverErr = Join-Path $ReportDir 'official-server.err.log'
  $serverProcess = Start-Process -FilePath (Resolve-Program 'npm') `
    -ArgumentList @('start') `
    -WorkingDirectory $BenchmarkRepo `
    -RedirectStandardOutput $serverOut `
    -RedirectStandardError $serverErr `
    -WindowStyle Hidden `
    -PassThru
  Add-Step 'server' 'started' "pid=$($serverProcess.Id); stdout=$serverOut; stderr=$serverErr"
  Start-Sleep -Seconds 5

  try {
    $benchArgs = @('run', 'bench', '--', '--framework') + $frameworkArgs + @('--chromeBinary', $ChromeBinary)
    if ($Benchmarks.Count -gt 0) {
      $benchArgs += '--benchmark'
      $benchArgs += $Benchmarks
    }
    if ($Count -gt 0) {
      $benchArgs += @('--count', $Count.ToString())
    }
    if ($Headless) {
      $benchArgs += '--headless'
    }
    if ($NoThrottling) {
      $benchArgs += '--nothrottling'
    }
    Invoke-LoggedProcess -Name 'official-bench' -WorkingDirectory $BenchmarkRepo -Program 'npm' -ProgramArgs $benchArgs | Out-Null
  } finally {
    if ($SkipResults) {
      Stop-OfficialServer -Process $serverProcess -Reason 'benchmark finished'
    }
  }
} else {
  Add-Step 'bench' 'skipped' 'SkipBench was set'
}

if (-not $SkipResults) {
  try {
    Invoke-LoggedProcess -Name 'official-results' -WorkingDirectory $BenchmarkRepo -Program 'npm' -ProgramArgs @('run', 'results') | Out-Null
  } finally {
    Stop-OfficialServer -Process $serverProcess -Reason 'results finished'
  }
} else {
  Add-Step 'results' 'skipped' 'SkipResults was set'
}

$resultsDir = Join-Path $BenchmarkRepo 'webdriver-ts/results'
if (Test-Path $resultsDir) {
  $dest = Join-Path $ReportDir 'results'
  if (Test-Path $dest) {
    Remove-Item -LiteralPath $dest -Recurse -Force
  }
  Copy-Item -Path $resultsDir -Destination $dest -Recurse -Force
  Add-Step 'copy-results' 'completed' $dest
}

$distDir = Join-Path $BenchmarkRepo 'webdriver-ts-results/dist'
if (Test-Path $distDir) {
  $dest = Join-Path $ReportDir 'results-dist'
  if (Test-Path $dest) {
    Remove-Item -LiteralPath $dest -Recurse -Force
  }
  Copy-Item -Path $distDir -Destination $dest -Recurse -Force
  Add-Step 'copy-results-dist' 'completed' $dest
}

$statusPath = Write-Status 'completed'
Write-Output $statusPath
