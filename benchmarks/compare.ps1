param(
 [string]$BuildRoot='D:/BuildCache/glory-framework-compare',
 [string]$SourceRoot='',
 [string]$OutDir='',
 [string]$Browser='C:/Program Files/Google/Chrome/Application/chrome.exe',
 [string]$Order='leptos,glory,dioxus',
 [int]$Port=8790,
 [switch]$SkipBuild
)
$ErrorActionPreference='Stop'
$BuildRoot=[IO.Path]::GetFullPath($BuildRoot)
if(-not $BuildRoot.Replace('\','/').StartsWith('D:/BuildCache/',[StringComparison]::OrdinalIgnoreCase)){throw 'BuildRoot must be D:/BuildCache'}
if(-not $OutDir){$OutDir="$BuildRoot/round-$(Get-Date -Format yyyyMMdd-HHmmss-fff)"}
$saved=@{}
foreach($key in @('TEMP','TMP','PYTHONPYCACHEPREFIX','NODE_COMPILE_CACHE')){$saved[$key]=[Environment]::GetEnvironmentVariable($key,'Process')}
$server=$null
try{
 New-Item -ItemType Directory -Force -Path "$BuildRoot/tmp","$BuildRoot/pycache" | Out-Null
 $env:TEMP="$BuildRoot/tmp"; $env:TMP=$env:TEMP
 $env:PYTHONPYCACHEPREFIX="$BuildRoot/pycache"; $env:NODE_COMPILE_CACHE="$BuildRoot/node-cache"
 if(-not $SkipBuild){& "$PSScriptRoot/build.ps1" -BuildRoot $BuildRoot -SourceRoot $SourceRoot}
 if(Get-NetTCPConnection -State Listen -LocalPort $Port -ErrorAction SilentlyContinue){throw "Port $Port is already owned"}
 $python=(python -c 'import sys;print(sys.executable)').Trim()
 $server=Start-Process -FilePath $python -ArgumentList @('-m','http.server',"$Port",'--bind','127.0.0.1','--directory',"$BuildRoot/site") -WindowStyle Hidden -PassThru -RedirectStandardOutput "$BuildRoot/server.stdout" -RedirectStandardError "$BuildRoot/server.stderr"
 $ready=$false
 for($attempt=0;$attempt -lt 30;$attempt++){
  if($server.HasExited){throw 'HTTP fixture exited during startup'}
  try{[void](Invoke-WebRequest -Uri "http://127.0.0.1:$Port/glory/" -TimeoutSec 1);$ready=$true;break}catch{Start-Sleep -Milliseconds 100}
 }
 if(-not $ready){throw 'HTTP fixture did not become ready'}
 node "$PSScriptRoot/run-browser.cjs" --base-url "http://127.0.0.1:$Port" --out $OutDir --browser $Browser --order $Order
 if($LASTEXITCODE -ne 0){throw "Browser checks failed; see $OutDir"}
 Write-Output "Raw comparison: $OutDir/evidence.json"
}finally{
 if($server -and -not $server.HasExited){Stop-Process -Id $server.Id}
 foreach($key in $saved.Keys){
  if($null -eq $saved[$key]){Remove-Item -LiteralPath "Env:$key" -ErrorAction SilentlyContinue}
  else{[Environment]::SetEnvironmentVariable($key,$saved[$key],'Process')}
 }
}
