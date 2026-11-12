# Measures write I/O of a Fob package on a hosted runner, phase by phase.
# Each phase records a WPR trace (DiskIO + FileIO) and a typeperf log, then
# tools/disk-writes sums them. Nothing but this runner's own files is read.
# usage: run.ps1 <fob.exe> <evidence dir> <analyzer.dll> <baseline min> <phase min> <helper min>
param(
  [Parameter(Mandatory)][string]$FobExe,
  [Parameter(Mandatory)][string]$Out,
  [Parameter(Mandatory)][string]$Analyzer,
  [int]$BaselineMin = 5,
  [int]$PhaseMin = 10,
  [int]$HelperMin = 8
)
$ErrorActionPreference = "Stop"
$win32 = Join-Path $PSScriptRoot "..\package-smoke\win32.ps1"
$work = Join-Path $env:RUNNER_TEMP "disk-writes"
New-Item -ItemType Directory -Force $work, $Out | Out-Null
$cDisk = (Get-Partition -DriveLetter C).DiskNumber
$configDir = Join-Path $env:APPDATA "com.hcb.fob"
$webviewDir = Join-Path $env:LOCALAPPDATA "com.hcb.fob"
"C: is disk $cDisk; work dir $work"
foreach ($d in $configDir, $webviewDir) { if (Test-Path $d) { throw "profile not fresh: $d exists" } }

function Fob { Get-Process fob -ErrorAction SilentlyContinue }
function Io {
  # GetProcessIoCounters totals (all I/O, cached or not) of the Fob processes.
  Get-CimInstance Win32_Process |
    Where-Object { $_.Name -in "fob.exe", "msedgewebview2.exe", "fob-sensors.exe" } |
    ForEach-Object { [ordered]@{ name = $_.Name; pid = $_.ProcessId; parent = $_.ParentProcessId
        write_bytes = $_.WriteTransferCount; write_ops = $_.WriteOperationCount
        other_bytes = $_.OtherTransferCount; fob_profile = [bool]($_.CommandLine -match "com\.hcb\.fob") } }
}
function Sizes {
  foreach ($d in $configDir, $webviewDir, "$env:ProgramFiles\Fob") {
    if (Test-Path $d) {
      Get-ChildItem $d -Recurse -File -Force -ErrorAction SilentlyContinue |
        Group-Object { $_.FullName.Substring($d.Length).Split("\")[1..3] -join "\" } |
        ForEach-Object { [ordered]@{ root = $d.Replace($env:USERPROFILE, "%USERPROFILE%"); part = $_.Name
            files = $_.Count; bytes = ($_.Group | Measure-Object Length -Sum).Sum } }
    }
  }
}
function Windows {
  $p = Fob
  if ($p) { powershell -NoProfile -ExecutionPolicy Bypass -File $win32 windows $p.Id | ConvertFrom-Json | Where-Object visible }
}
function Hotkey {
  # Ctrl+Alt+F9 (seeded as behavior.miniHotkey); RegisterHotKey sees synthetic input.
  Add-Type -Namespace K -Name B -MemberDefinition '[DllImport("user32.dll")] public static extern void keybd_event(byte k, byte s, uint f, UIntPtr e);'
  foreach ($k in 0x11, 0x12, 0x78) { [K.B]::keybd_event($k, 0, 0, [UIntPtr]::Zero) }
  foreach ($k in 0x78, 0x12, 0x11) { [K.B]::keybd_event($k, 0, 2, [UIntPtr]::Zero) }
}
function Phase([string]$Label, [int]$Minutes, [scriptblock]$Start) {
  $etl = Join-Path $work "$Label.etl"
  $csv = Join-Path $work "$Label.csv"
  wpr -start DiskIO -start FileIO -filemode -recordtempto $work
  if ($LASTEXITCODE) { throw "wpr start failed: $LASTEXITCODE" }
  $tp = Start-Process typeperf -PassThru -WindowStyle Hidden -ArgumentList @(
    '"\LogicalDisk(C:)\Disk Write Bytes/sec"', '"\PhysicalDisk(_Total)\Disk Write Bytes/sec"',
    "-si", "1", "-sc", ($Minutes * 60), "-f", "CSV", "-o", "`"$csv`"", "-y")
  $begin = @(Io)
  if ($Start) { & $Start }
  Start-Sleep -Seconds ($Minutes * 60)
  $windows = @(Windows)
  $end = @(Io)
  wpr -stop $etl
  if ($LASTEXITCODE) { throw "wpr stop failed: $LASTEXITCODE" }
  $tp | Wait-Process -Timeout 60 -ErrorAction SilentlyContinue
  "$Label etl $([math]::Round((Get-Item $etl).Length / 1MB)) MB"
  dotnet $Analyzer phase $etl $Label (Join-Path $Out "$Label.json") $cDisk $env:USERPROFILE $csv
  if ($LASTEXITCODE) { throw "analyzer failed: $LASTEXITCODE" }
  [ordered]@{ label = $Label; windows = $windows; io_begin = $begin; io_end = $end; sizes = @(Sizes) } |
    ConvertTo-Json -Depth 6 | Set-Content (Join-Path $Out "$Label-state.json")
  Remove-Item $etl, $csv -ErrorAction SilentlyContinue
}

# 1. No Fob: what this runner writes on its own.
Phase "baseline" $BaselineMin $null
# 2. First launch, default settings: start in tray, no window shown.
Phase "tray" $PhaseMin { Start-Process $FobExe | Out-Null; Start-Sleep 5 }
# 3. A second launch brings the main window forward (Processes tab).
Phase "main" $PhaseMin { Start-Process $FobExe | Out-Null; Start-Sleep 5 }
# 4. Restart with a mini window hotkey and pin, then show the mini window.
Fob | Stop-Process -Force; Start-Sleep 5
$settings = Join-Path $configDir "settings.json"
# Fob saves settings.json only after a change; a partial file is merged over the defaults.
$json = if (Test-Path $settings) { Get-Content $settings -Raw | ConvertFrom-Json } else { [pscustomobject]@{} }
if (-not $json.behavior) { $json | Add-Member -Force behavior ([pscustomobject]@{}) }
$json.behavior | Add-Member -Force miniHotkey "Ctrl+Alt+F9"
$json.behavior | Add-Member -Force miniPinned $true
New-Item -ItemType Directory -Force $configDir | Out-Null
$json | ConvertTo-Json -Depth 20 | Set-Content $settings -Encoding utf8
Phase "mini" $PhaseMin { Start-Process $FobExe | Out-Null; Start-Sleep 20; Hotkey; Start-Sleep 3 }
# 5. Install the sensor helper (this runner is already elevated), then the main window.
Fob | Stop-Process -Force; Start-Sleep 5
# Fob runs PawnIO's installer with its window (the user sees it), which would
# wait forever here; install it silently first so Fob skips that step.
function Bounded([string]$Exe, [string[]]$Arguments, [int]$Seconds) {
  $p = Start-Process $Exe -ArgumentList $Arguments -PassThru
  if (-not $p.WaitForExit($Seconds * 1000)) { $p | Stop-Process -Force; return "timeout" }
  return $p.ExitCode
}
$pawnio = Join-Path (Split-Path $FobExe) "PawnIO\PawnIO_setup.exe"
if (Test-Path $pawnio) { "PawnIO silent install: $(Bounded $pawnio @("-install", "-silent") 180)" }
"sensors-setup: $(Bounded $FobExe @("--sensors-setup") 180)"
Start-Sleep 10
$helperUp = [bool](Get-Process fob-sensors -ErrorAction SilentlyContinue)
"fob-sensors running: $helperUp"
if (-not $helperUp) { Write-Warning "helper phase runs without the helper" }
Phase "helper" $HelperMin { Start-Process $FobExe | Out-Null; Start-Sleep 15; Start-Process $FobExe | Out-Null; Start-Sleep 5 }
Fob | Stop-Process -Force
$jsons = "baseline", "tray", "main", "mini", "helper" | ForEach-Object { Join-Path $Out "$_.json" } | Where-Object { Test-Path $_ }
dotnet $Analyzer report (Join-Path $Out "report.md") @jsons
