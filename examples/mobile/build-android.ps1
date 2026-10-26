param(
    [ValidateSet('x86_64-linux-android','aarch64-linux-android')][string]$Target = 'x86_64-linux-android',
    [string]$BuildRoot = 'D:/BuildCache/glory-mobile',
    [string]$Sdk = $env:ANDROID_HOME,
    [string]$Ndk = $env:ANDROID_NDK_HOME,
    [string]$JavaHome = 'C:/Program Files/Android/Android Studio/jbr',
    [switch]$Release
)
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
. "$PSScriptRoot/build-paths.ps1"
$build = Resolve-MobileBuildRoot $BuildRoot
$saved = @{}
$targetKey = $Target.Replace('-','_').ToUpperInvariant()
$names = @('CARGO_HOME','CARGO_TARGET_DIR','TEMP','TMP','TMPDIR','PYTHONPYCACHEPREFIX','JAVA_HOME','JAVA_TOOL_OPTIONS',"CARGO_TARGET_${targetKey}_LINKER", "CC_$($Target.Replace('-','_'))")
foreach ($name in $names) { $saved[$name] = [Environment]::GetEnvironmentVariable($name,'Process') }
Push-Location $PSScriptRoot
try {
    if ([string]::IsNullOrWhiteSpace($Sdk) -or -not (Test-Path -LiteralPath "$Sdk/platforms")) { throw 'Set ANDROID_HOME or -Sdk to an installed Android SDK' }
    if (-not (Test-Path -LiteralPath "$JavaHome/bin/java.exe")) { throw 'Set -JavaHome to an installed JDK' }
    $env:CARGO_HOME = 'D:/BuildCache/glory-semantic-ui/cargo'
    $env:CARGO_TARGET_DIR = "$build/target"
    $env:TEMP = "$build/tmp"; $env:TMP = $env:TEMP; $env:TMPDIR = $env:TEMP
    $env:PYTHONPYCACHEPREFIX = "$build/pycache"
    $env:JAVA_HOME = $JavaHome
    $env:JAVA_TOOL_OPTIONS = "-Djava.io.tmpdir=$build/tmp -Duser.home=$build/java-home"
    New-Item -ItemType Directory -Force $env:TEMP,"$build/java-home","$build/android-home" | Out-Null
    if ([string]::IsNullOrWhiteSpace($Ndk) -or -not (Test-Path -LiteralPath $Ndk)) {
        $Ndk = Get-ChildItem -LiteralPath "$Sdk/ndk" -Directory | Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1 -ExpandProperty FullName
        Write-Output "Selecting installed NDK: $Ndk"
    }
    $linker = Join-Path $Ndk "toolchains/llvm/prebuilt/windows-x86_64/bin/$($Target)28-clang.cmd"
    if (-not (Test-Path -LiteralPath $linker)) { throw "Missing NDK compiler: $linker" }
    [Environment]::SetEnvironmentVariable("CARGO_TARGET_${targetKey}_LINKER",$linker,'Process')
    [Environment]::SetEnvironmentVariable("CC_$($Target.Replace('-','_'))",$linker,'Process')
    $metadata = cargo metadata --manifest-path Cargo.toml --no-deps --format-version 1 | ConvertFrom-Json
    if ($LASTEXITCODE -or [IO.Path]::GetFullPath($metadata.target_directory) -ne [IO.Path]::GetFullPath($env:CARGO_TARGET_DIR)) { throw 'Unexpected Cargo target' }
    $options = @('build','--locked','--lib','--target',$Target)
    if ($Release) { $options += '--release' }
    cargo @options
    if ($LASTEXITCODE) { throw 'Android library build failed' }
    $profile = if ($Release) { 'release' } else { 'debug' }
    $abi = if ($Target -eq 'x86_64-linux-android') { 'x86_64' } else { 'arm64-v8a' }
    $library = "$env:CARGO_TARGET_DIR/$Target/$profile/libglory_mobile_counter.so"
    $llvm = Join-Path $Ndk 'toolchains/llvm/prebuilt/windows-x86_64/bin'
    $header = & "$llvm/llvm-readelf.exe" -h $library
    if ($LASTEXITCODE) { throw 'Cannot inspect Android ELF' }
    $machine = $header | Where-Object { $_ -match 'Machine:' } | Select-Object -First 1
    $expectedMachine = if ($abi -eq 'arm64-v8a') { 'AArch64' } else { 'X86-64' }
    if (-not $machine.Contains($expectedMachine)) { throw "Unexpected ELF machine: $machine" }
    $symbols = & "$llvm/llvm-nm.exe" -D --defined-only $library
    if ($LASTEXITCODE -or -not ($symbols -match '\bANativeActivity_onCreate$') -or -not ($symbols -match '\bandroid_main$')) { throw 'Android library does not export its required Activity entry points' }
    $stage = "$build/package-$Target-$profile"
    New-Item -ItemType Directory -Force "$stage/lib/$abi" | Out-Null
    Copy-Item -LiteralPath $library -Destination "$stage/lib/$abi/"
    $tools = Get-ChildItem -LiteralPath "$Sdk/build-tools" -Directory | Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1 -ExpandProperty FullName
    $platform = Get-ChildItem -LiteralPath "$Sdk/platforms" -Directory | Sort-Object { [version](($_.Name -replace '^android-','') + '.0') } -Descending | Select-Object -First 1 -ExpandProperty FullName
    $unsigned = "$stage/unsigned.apk"
    & "$tools/aapt.exe" package -f -M "$PSScriptRoot/AndroidManifest.xml" -I "$platform/android.jar" -F $unsigned
    if ($LASTEXITCODE) { throw 'Android manifest packaging failed' }
    & "$JavaHome/bin/jar.exe" uf $unsigned -C $stage lib
    if ($LASTEXITCODE) { throw 'Native library packaging failed' }
    $aligned = "$stage/aligned.apk"
    & "$tools/zipalign.exe" -f -P 16 4 $unsigned $aligned
    if ($LASTEXITCODE) { throw 'APK alignment failed' }
    $key = "$build/android-home/debug.keystore"
    if (-not (Test-Path -LiteralPath $key)) {
        & "$JavaHome/bin/keytool.exe" -genkeypair -keystore $key -storepass android -alias androiddebugkey -keypass android -dname 'CN=Glory Debug' -keyalg RSA -validity 10000
        if ($LASTEXITCODE) { throw 'Debug key generation failed' }
    }
    $apk = "$stage/glory-counter.apk"
    & "$tools/apksigner.bat" sign --ks $key --ks-pass pass:android --key-pass pass:android --out $apk $aligned
    if ($LASTEXITCODE) { throw 'APK debug signing failed' }
    & "$tools/apksigner.bat" verify $apk
    if ($LASTEXITCODE) { throw 'APK verification failed' }
    @{ target=$Target; profile=$profile; abi=$abi; elf_machine=$machine.Trim(); entrypoints_verified=$true; apk=$apk; sha256=(Get-FileHash -LiteralPath $apk -Algorithm SHA256).Hash.ToLowerInvariant(); signing='debug-only' } | ConvertTo-Json | Set-Content -LiteralPath "$stage/build.json"
    Write-Output "Android APK: $apk"
}
finally {
    Pop-Location
    foreach ($name in $saved.Keys) { [Environment]::SetEnvironmentVariable($name,$saved[$name],'Process') }
}
