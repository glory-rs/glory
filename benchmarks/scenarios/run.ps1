param([string]$BuildRoot="D:/BuildCache/glory-portfolio-$(Get-Date -Format yyyyMMdd-HHmmss)",[int]$Port=29045,[switch]$SkipBuild)
$ErrorActionPreference='Stop'
$BuildRoot=[IO.Path]::GetFullPath($BuildRoot)
if(-not $BuildRoot.Replace('\','/').StartsWith('D:/BuildCache/',[StringComparison]::OrdinalIgnoreCase)){throw 'Use D:/BuildCache'}
if(Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue){throw 'Port already in use'}
$saved=@{};foreach($key in @('TEMP','TMP','PYTHONPYCACHEPREFIX')){$saved[$key]=[Environment]::GetEnvironmentVariable($key,'Process')}
$taskServer=$null
try{
 New-Item -ItemType Directory -Force -Path "$BuildRoot/tmp"|Out-Null
 $env:TEMP="$BuildRoot/tmp";$env:TMP=$env:TEMP;$env:PYTHONPYCACHEPREFIX="$BuildRoot/pycache"
 if(-not $SkipBuild){python "$PSScriptRoot/freeze.py" --out "$BuildRoot/source";if($LASTEXITCODE -ne 0){throw 'Freeze failed'};& "$PSScriptRoot/build.ps1" -BuildRoot $BuildRoot -SourceRoot "$BuildRoot/source"}
 if(-not(Test-Path "$BuildRoot/site/dioxus-row/app_bg.wasm")){throw 'Missing independent sites'}
 $taskServer=Start-Process -FilePath (Get-Command python).Source -ArgumentList @('-m','http.server',"$Port",'--bind','127.0.0.1','--directory',('"{0}"' -f $BuildRoot)) -WindowStyle Hidden -PassThru -RedirectStandardOutput "$BuildRoot/server.stdout" -RedirectStandardError "$BuildRoot/server.stderr"
 $ready=$false;for($attempt=0;$attempt -lt 30;$attempt++){try{$null=Invoke-WebRequest "http://127.0.0.1:$Port/site/glory/" -TimeoutSec 1;$ready=$true;break}catch{Start-Sleep -Milliseconds 100}}
 if(-not $ready){throw 'Server failed to start'}
 node "$PSScriptRoot/compare.cjs" --out "$BuildRoot/results" --base "http://127.0.0.1:$Port" --pin "$PSScriptRoot/pin-browser.ps1"
 if($LASTEXITCODE -ne 0){throw 'Comparison failed'}
}finally{
 if($taskServer){$taskProcess=Get-CimInstance Win32_Process -Filter "ProcessId=$($taskServer.Id)";if($taskProcess -and $taskProcess.CommandLine -like "*-m http.server $Port*" -and $taskProcess.CommandLine.Contains($BuildRoot)){Stop-Process -Id $taskServer.Id}}
 foreach($key in $saved.Keys){[Environment]::SetEnvironmentVariable($key,$saved[$key],'Process')}
}
