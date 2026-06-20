# Measure raw wasm sizes for representative Glory browser builds.
# Optional: if Binaryen's wasm-opt is on PATH, release artifacts are also
# optimized with `wasm-opt -Oz` into target/wasm-size.
param(
  [switch]$SkipBuild,
  [switch]$Json
)

$ErrorActionPreference = 'Stop'
$Root = Resolve-Path (Join-Path $PSScriptRoot '..')
$Target = 'wasm32-unknown-unknown'

$candidateCases = @(
  [PSCustomObject]@{
    Name = '_test-size'
    Variant = 'minimal'
    Manifest = 'examples/_test-size/Cargo.toml'
    Features = @()
    Wasm = 'examples/_test-size/target/wasm32-unknown-unknown/{0}/_test-size.wasm'
  },
  [PSCustomObject]@{
    Name = 'counter'
    Variant = 'web-csr'
    Manifest = 'examples/counter/Cargo.toml'
    Features = @()
    Wasm = 'examples/counter/target/wasm32-unknown-unknown/{0}/counter.wasm'
  },
  [PSCustomObject]@{
    Name = 'router-basic'
    Variant = 'web-csr+routing'
    Manifest = 'examples/router-basic/Cargo.toml'
    Features = @()
    Wasm = 'examples/router-basic/target/wasm32-unknown-unknown/{0}/router-basic.wasm'
  },
  [PSCustomObject]@{
    Name = 'todomvc-fullstack'
    Variant = 'web-csr+server-fn'
    Manifest = 'examples/todomvc-fullstack/Cargo.toml'
    Features = @('--features', 'web-csr')
    Wasm = 'examples/todomvc-fullstack/target/wasm32-unknown-unknown/{0}/todomvc-fullstack.wasm'
  }
)

$skipped = @()
$cases = @()
foreach ($case in $candidateCases) {
  $manifest = Join-Path $Root $case.Manifest
  if (Test-Path $manifest) {
    $cases += $case
  } else {
    $skipped += [PSCustomObject]@{
      Kind = 'case'
      Name = $case.Name
      Reason = "manifest not found: $($case.Manifest)"
    }
  }
}

function Invoke-CargoBuild($case, [string]$profile) {
  $args = @('build', '--manifest-path', (Join-Path $Root $case.Manifest), '--target', $Target)
  if ($profile -eq 'release') {
    $args += '--release'
  }
  $args += $case.Features
  & cargo @args
  if ($LASTEXITCODE -ne 0) {
    throw "cargo build failed for $($case.Name) $profile"
  }
}

function New-SizeRow($case, [string]$profile, [string]$kind, [string]$path) {
  $item = Get-Item $path
  [PSCustomObject]@{
    Example = $case.Name
    Variant = $case.Variant
    Profile = $profile
    Kind = $kind
    Bytes = $item.Length
    KiB = [Math]::Round($item.Length / 1KB, 1)
    MiB = [Math]::Round($item.Length / 1MB, 2)
    Path = (Resolve-Path $path).Path
  }
}

function Write-MarkdownTable($rows) {
  '| Example | Variant | Profile | Kind | Bytes | KiB | MiB |'
  '| --- | --- | --- | --- | ---: | ---: | ---: |'
  foreach ($row in $rows) {
    '| {0} | {1} | {2} | {3} | {4} | {5} | {6} |' -f `
      $row.Example, $row.Variant, $row.Profile, $row.Kind, $row.Bytes, $row.KiB, $row.MiB
  }
}

if (-not $SkipBuild) {
  foreach ($case in $cases) {
    Invoke-CargoBuild $case 'debug'
    Invoke-CargoBuild $case 'release'
  }
}

$rows = @()
foreach ($case in $cases) {
  foreach ($profile in @('debug', 'release')) {
    $relative = $case.Wasm -f $profile
    $path = Join-Path $Root $relative
    if (Test-Path $path) {
      $rows += New-SizeRow $case $profile 'raw cargo wasm' $path
    } else {
      $skipped += [PSCustomObject]@{
        Kind = 'artifact'
        Name = "$($case.Name) $profile"
        Reason = "wasm artifact not found: $relative"
      }
    }
  }
}

$wasmOpt = Get-Command wasm-opt -ErrorAction SilentlyContinue
if ($wasmOpt) {
  $outDir = Join-Path $Root 'target/wasm-size'
  New-Item -ItemType Directory -Force -Path $outDir | Out-Null

  foreach ($case in $cases) {
    $source = Join-Path $Root ($case.Wasm -f 'release')
    if (-not (Test-Path $source)) {
      $skipped += [PSCustomObject]@{
        Kind = 'artifact'
        Name = "$($case.Name) release"
        Reason = "wasm-opt source not found: $($case.Wasm -f 'release')"
      }
      continue
    }
    $out = Join-Path $outDir "$($case.Name)-Oz.wasm"
    & $wasmOpt.Source -Oz $source -o $out
    if ($LASTEXITCODE -ne 0) {
      throw "wasm-opt failed for $($case.Name)"
    }
    $rows += New-SizeRow $case 'release' 'wasm-opt -Oz' $out
  }
} else {
  $skipped += [PSCustomObject]@{
    Kind = 'tool'
    Name = 'wasm-opt'
    Reason = 'wasm-opt was not found on PATH; wasm-opt -Oz rows skipped'
  }
}

$wasmBindgen = Get-Command wasm-bindgen -ErrorAction SilentlyContinue
if ($wasmBindgen) {
  $outRoot = Join-Path $Root 'target/wasm-size'
  New-Item -ItemType Directory -Force -Path $outRoot | Out-Null

  foreach ($case in $cases) {
    $source = Join-Path $Root ($case.Wasm -f 'release')
    if (-not (Test-Path $source)) {
      $skipped += [PSCustomObject]@{
        Kind = 'artifact'
        Name = "$($case.Name) release"
        Reason = "wasm-bindgen source not found: $($case.Wasm -f 'release')"
      }
      continue
    }
    $outDir = Join-Path $outRoot "$($case.Name)-bindgen"
    if (Test-Path $outDir) {
      Remove-Item -LiteralPath $outDir -Recurse -Force
    }
    New-Item -ItemType Directory -Force -Path $outDir | Out-Null
    & $wasmBindgen.Source --target web --out-dir $outDir $source
    if ($LASTEXITCODE -ne 0) {
      throw "wasm-bindgen failed for $($case.Name)"
    }
    $bindgenWasm = Get-ChildItem -Path $outDir -Filter '*_bg.wasm' | Select-Object -First 1
    if ($bindgenWasm) {
      $rows += New-SizeRow $case 'release' 'wasm-bindgen web' $bindgenWasm.FullName
    } else {
      $skipped += [PSCustomObject]@{
        Kind = 'case'
        Name = $case.Name
        Reason = 'wasm-bindgen did not produce a *_bg.wasm file'
      }
    }
  }
} else {
  $skipped += [PSCustomObject]@{
    Kind = 'tool'
    Name = 'wasm-bindgen'
    Reason = 'wasm-bindgen was not found on PATH; bindgen/download-size rows skipped'
  }
}

if ($Json) {
  [PSCustomObject]@{
    Generated = (Get-Date -Format s)
    Target = $Target
    Tools = [PSCustomObject]@{
      WasmOpt = [bool]$wasmOpt
      WasmBindgen = [bool]$wasmBindgen
    }
    Skipped = $skipped
    Rows = $rows
  } | ConvertTo-Json -Depth 6
} else {
  Write-MarkdownTable $rows
  if ($skipped.Count -gt 0) {
    ''
    'Skipped:'
    foreach ($item in $skipped) {
      "- $($item.Kind) $($item.Name): $($item.Reason)"
    }
  }
}
