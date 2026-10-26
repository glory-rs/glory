param(
    [string]$BuildRoot = 'D:/BuildCache/glory-mobile',
    [string]$Sdk = $env:ANDROID_HOME,
    [string]$Image = 'system-images/android-36.1/google_apis_playstore/x86_64',
    [ValidateRange(5554,5680)][int]$Port = 5580
)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/build-paths.ps1"
$build = Resolve-MobileBuildRoot $BuildRoot
if ($Port % 2) { throw 'Android emulator console port must be even' }
$imageRoot = (Resolve-Path -LiteralPath (Join-Path $Sdk $Image)).Path
if (-not (Test-Path -LiteralPath "$imageRoot/system.img")) { throw 'Install the SDK x86_64 system image first' }
$saved = @{}
foreach ($name in @('ANDROID_USER_HOME','ANDROID_EMULATOR_HOME','ANDROID_AVD_HOME','TEMP','TMP')) {
    $saved[$name] = [Environment]::GetEnvironmentVariable($name,'Process')
}
try {
    $env:ANDROID_USER_HOME = "$build/android-home"
    $env:ANDROID_EMULATOR_HOME = "$build/emulator-home"
    $env:ANDROID_AVD_HOME = "$build/avd"
    $env:TEMP = "$build/tmp"; $env:TMP=$env:TEMP
    $avd = "$env:ANDROID_AVD_HOME/Glory_Counter.avd"
    New-Item -ItemType Directory -Force $env:ANDROID_USER_HOME,$env:ANDROID_EMULATOR_HOME,$avd,$env:TEMP | Out-Null
    if (-not (Test-Path -LiteralPath "$avd/config.ini")) {
        @"
AvdId=Glory_Counter
avd.ini.displayname=Glory Counter Acceptance
avd.ini.encoding=UTF-8
abi.type=x86_64
hw.cpu.arch=x86_64
hw.cpu.ncore=2
hw.ramSize=2048
hw.lcd.width=1080
hw.lcd.height=2400
hw.lcd.density=420
hw.keyboard=yes
hw.gpu.enabled=yes
hw.gpu.mode=software
disk.dataPartition.size=2G
image.sysdir.1=$imageRoot
tag.id=google_apis_playstore
"@ | Set-Content -LiteralPath "$avd/config.ini"
    }
    @('avd.ini.encoding=UTF-8',"path=$avd",'target=android-36.1') | Set-Content -LiteralPath "$env:ANDROID_AVD_HOME/Glory_Counter.ini"
    $process = Start-Process "$Sdk/emulator/emulator.exe" -ArgumentList @('-avd','Glory_Counter','-no-window','-no-audio','-no-snapshot','-no-boot-anim','-gpu','software','-cores','2','-port',$Port) -WindowStyle Hidden -PassThru -RedirectStandardOutput "$build/emulator-stdout.log" -RedirectStandardError "$build/emulator-stderr.log"
    $process.Id | Set-Content -LiteralPath "$build/emulator.pid"
    Write-Output "Owned emulator: emulator-$Port; launcher PID $($process.Id); data $avd"
}
finally {
    foreach ($name in $saved.Keys) { [Environment]::SetEnvironmentVariable($name,$saved[$name],'Process') }
}
