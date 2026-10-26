param(
    [ValidateSet('web','desktop')][string]$Platform = 'web',
    [switch]$Run,
    [string]$BuildRoot = 'D:/BuildCache/glory-semantic-ui',
    [string]$Bindgen = ''
)
$ErrorActionPreference = 'Stop'
$savedRoot = $env:GLORY_BUILD_ROOT
try {
    $env:GLORY_BUILD_ROOT = $BuildRoot
    $command = if ($Run) { 'run' } else { 'build' }
    $forward = @($command,'--platform',$Platform)
    if ($Bindgen) { $forward += @('--bindgen',$Bindgen) }
    & "$PSScriptRoot/bin/glory.ps1" @forward
}
finally { $env:GLORY_BUILD_ROOT = $savedRoot }
