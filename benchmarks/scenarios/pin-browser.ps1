param([Parameter(Mandatory=$true)][string]$ProfileRoot,[Parameter(Mandatory=$true)][string]$Out)
$ErrorActionPreference='Stop'
$taskProfile=[IO.Path]::GetFullPath($ProfileRoot).Replace('\','/').TrimEnd('/')
if(-not $taskProfile.StartsWith('D:/BuildCache/',[StringComparison]::OrdinalIgnoreCase)){throw 'Profile must be on D:'}
$records=@();$renderers=0
foreach($item in @(Get-CimInstance Win32_Process -Filter "Name='chrome.exe'")){
 if(-not $item.CommandLine -or $item.CommandLine.Replace('\','/').IndexOf($taskProfile,[StringComparison]::OrdinalIgnoreCase) -lt 0 -or $item.CommandLine -match 'crashpad-handler'){continue}
 try{$process=Get-Process -Id $item.ProcessId;$process.ProcessorAffinity=[IntPtr]65535;$process.PriorityClass=[Diagnostics.ProcessPriorityClass]::AboveNormal;$process.Refresh()
  if($process.ProcessorAffinity.ToInt64() -ne 65535 -or $process.PriorityClass -ne 'AboveNormal'){throw 'Pinning did not stick'}
 }catch{if(-not(Get-Process -Id $item.ProcessId -ErrorAction SilentlyContinue)){continue};throw}
 if($item.CommandLine -match '--type=renderer'){$renderers++}
 $records+=@{pid=$item.ProcessId;mask=$process.ProcessorAffinity.ToInt64();priority=$process.PriorityClass.ToString();command=$item.CommandLine}
}
if($renderers -lt 1){throw 'No owned renderer pinned'}
@{time=(Get-Date).ToUniversalTime().ToString('o');renderers=$renderers;processes=$records}|ConvertTo-Json -Depth 6 -Compress|Add-Content -LiteralPath $Out
