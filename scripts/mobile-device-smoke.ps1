# Run install/launch smoke checks for mobile artifacts produced by Glory.
param(
  [ValidateSet('android', 'ios', 'all')]
  [string]$Target = 'all',
  [string]$AndroidApk = '',
  [string]$AndroidPackage = 'com.example.mobile_counter',
  [string]$AndroidActivity = '.MainActivity',
  [string]$AndroidSerial = $env:GLORY_ANDROID_DEVICE,
  [switch]$AndroidReverseReload,
  [string]$ReloadPort = $env:GLORY_RELOAD_PORT,
  [string]$IosApp = '',
  [string]$IosBundleId = 'com.example.MobileCounter',
  [string]$IosDestination = $env:GLORY_IOS_DESTINATION,
  [string]$OutDir = 'target/mobile-device-smoke'
)

$ErrorActionPreference = 'Stop'
$Root = Resolve-Path (Join-Path $PSScriptRoot '..')
$ReportDir = Join-Path $Root $OutDir
New-Item -ItemType Directory -Force -Path $ReportDir | Out-Null
$AndroidReverseReloadEnabled = $AndroidReverseReload.IsPresent -or $env:GLORY_ANDROID_REVERSE_RELOAD -eq '1'

$checks = New-Object System.Collections.Generic.List[object]

function Add-Check([string]$name, [string]$status, [string]$detail) {
  $checks.Add([PSCustomObject]@{
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
    [string]$Program,
    [Parameter(Mandatory = $true)]
    [string[]]$ProgramArgs
  )

  $log = Join-Path $ReportDir "$Name.log"
  $resolvedProgram = Resolve-Program $Program
  & $resolvedProgram @ProgramArgs *>&1 | Tee-Object -FilePath $log
  if ($LASTEXITCODE -ne 0) {
    Add-Check $Name 'failed' $log
    throw "$Program $($ProgramArgs -join ' ') failed; see $log"
  }
  Add-Check $Name 'completed' $log
  return $log
}

function Write-Status([string]$status) {
  $path = Join-Path $ReportDir 'mobile-device-smoke.json'
  [PSCustomObject]@{
    Status = $status
    Generated = (Get-Date -Format s)
    Target = $Target
    Android = [PSCustomObject]@{
      Apk = $AndroidApk
      Package = $AndroidPackage
      Activity = $AndroidActivity
      Serial = $AndroidSerial
      Adb = Resolve-Adb
      ReverseReload = $AndroidReverseReloadEnabled
      ReloadPort = $ReloadPort
    }
    Ios = [PSCustomObject]@{
      App = $IosApp
      BundleId = $IosBundleId
      Destination = $IosDestination
    }
    Checks = $checks
  } | ConvertTo-Json -Depth 6 | Set-Content -Path $path -Encoding utf8
  return $path
}

function Has-Command([string]$name) {
  [bool](Get-Command $name -ErrorAction SilentlyContinue)
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

function Resolve-Adb {
  $command = Get-Command adb -ErrorAction SilentlyContinue
  if ($command -and $command.Source) {
    return $command.Source
  }

  foreach ($root in @($env:ANDROID_HOME, $env:ANDROID_SDK_ROOT)) {
    if (-not $root) {
      continue
    }
    $candidate = if ($IsWindows) {
      Join-Path $root 'platform-tools/adb.exe'
    } else {
      Join-Path $root 'platform-tools/adb'
    }
    if (Test-Path $candidate) {
      return (Resolve-Path $candidate).Path
    }
  }

  return ''
}

function Android-Args([string[]]$args) {
  if ($AndroidSerial -and $AndroidSerial.Trim()) {
    return @('-s', $AndroidSerial.Trim()) + $args
  }
  return $args
}

function Run-AndroidSmoke {
  $adb = Resolve-Adb
  if (-not $adb) {
    Add-Check 'android-adb' 'blocked' 'adb was not found on PATH or under ANDROID_HOME/ANDROID_SDK_ROOT/platform-tools'
    return
  }
  Add-Check 'android-adb' 'completed' $adb
  $devicesRaw = & $adb devices
  $devices = $devicesRaw | Where-Object { $_ -match "`tdevice$" }
  if (-not $devices -or $devices.Count -eq 0) {
    Add-Check 'android-device' 'blocked' 'No adb device or emulator is online'
    return
  }
  Add-Check 'android-device' 'completed' ($devices -join '; ')

  if ($AndroidReverseReloadEnabled) {
    if (-not $ReloadPort) {
      Add-Check 'android-reload-reverse' 'blocked' 'Pass -ReloadPort or set GLORY_RELOAD_PORT before requesting Android reload reverse'
    } else {
      Invoke-LoggedProcess -Name 'android-reload-reverse' -Program $adb -ProgramArgs (Android-Args @('reverse', "tcp:$ReloadPort", "tcp:$ReloadPort")) | Out-Null
    }
  }

  if (-not $AndroidApk) {
    Add-Check 'android-apk' 'blocked' 'Pass -AndroidApk with a built APK from glory bundle --target android'
    return
  }
  if (-not (Test-Path $AndroidApk)) {
    Add-Check 'android-apk' 'failed' "APK not found: $AndroidApk"
    return
  }

  Invoke-LoggedProcess -Name 'android-install' -Program $adb -ProgramArgs (Android-Args @('install', '-r', $AndroidApk)) | Out-Null
  $activity = if ($AndroidActivity.StartsWith('.')) { "$AndroidPackage/$AndroidPackage$AndroidActivity" } else { "$AndroidPackage/$AndroidActivity" }
  Invoke-LoggedProcess -Name 'android-launch' -Program $adb -ProgramArgs (Android-Args @('shell', 'am', 'start', '-W', '-n', $activity)) | Out-Null
  Invoke-LoggedProcess -Name 'android-window-dump' -Program $adb -ProgramArgs (Android-Args @('shell', 'dumpsys', 'window', 'displays')) | Out-Null
}

function Run-IosSmoke {
  if (-not $IsMacOS) {
    Add-Check 'ios-host' 'blocked' 'iOS simulator/device smoke requires macOS'
    return
  }
  foreach ($tool in @('xcrun', 'xcodebuild')) {
    if (-not (Has-Command $tool)) {
      Add-Check "ios-$tool" 'blocked' "$tool was not found on PATH"
      return
    }
  }
  if (-not $IosApp) {
    Add-Check 'ios-app' 'blocked' 'Pass -IosApp with a built .app bundle from glory bundle --target ios'
    return
  }
  if (-not (Test-Path $IosApp)) {
    Add-Check 'ios-app' 'failed' ".app bundle not found: $IosApp"
    return
  }

  $destination = if ($IosDestination -and $IosDestination.Trim()) { $IosDestination.Trim() } else { 'booted' }
  Invoke-LoggedProcess -Name 'ios-install' -Program 'xcrun' -ProgramArgs @('simctl', 'install', $destination, $IosApp) | Out-Null
  Invoke-LoggedProcess -Name 'ios-launch' -Program 'xcrun' -ProgramArgs @('simctl', 'launch', $destination, $IosBundleId) | Out-Null
}

Invoke-LoggedProcess -Name 'mobile-counter-host-check' -Program 'cargo' -ProgramArgs @('check', '--manifest-path', (Join-Path $Root 'examples/mobile-counter/Cargo.toml')) | Out-Null

if ($Target -eq 'android' -or $Target -eq 'all') {
  Run-AndroidSmoke
}
if ($Target -eq 'ios' -or $Target -eq 'all') {
  Run-IosSmoke
}

$blocked = $checks | Where-Object { $_.Status -eq 'blocked' }
$failed = $checks | Where-Object { $_.Status -eq 'failed' }
$status = if ($failed) { 'failed' } elseif ($blocked) { 'blocked' } else { 'completed' }
$statusPath = Write-Status $status
Write-Output $statusPath
