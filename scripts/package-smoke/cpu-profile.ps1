param(
    [string]$FobDir,
    [string]$PackageRoot,
    [string]$Evidence,
    [switch]$ValidateOnly
)
$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
$parseErrors = $null
[void][System.Management.Automation.Language.Parser]::ParseFile($PSCommandPath, [ref]$null, [ref]$parseErrors)
if ($parseErrors.Count) { throw "PowerShell syntax errors: $parseErrors" }
Add-Type -Path (Join-Path $PSScriptRoot 'cpu-profile.cs')
$checks = [FobStartup.Probe]::SelfTest()
Write-Host "CPU accounting and native preflight checks passed: $checks"
if ($ValidateOnly) { return }
if (-not $IsWindows -or $env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
    throw 'Full diagnostic is restricted to a GitHub-hosted Windows runner'
}
if (-not $FobDir -or -not $PackageRoot -or -not $Evidence) { throw 'Package and evidence directories are required' }
if (Get-Process -Name fob -ErrorAction SilentlyContinue) { throw 'An unrelated Fob already exists; refusing to kill or attach' }
[void](New-Item -ItemType Directory -Force $Evidence)
function Save($Name, $Value) {
    $Value | ConvertTo-Json -Depth 16 | Set-Content -Encoding utf8 (Join-Path $Evidence $Name)
}
function Hash($Path) { (Get-FileHash -Algorithm SHA256 -LiteralPath $Path).Hash.ToLowerInvariant() }
$source = '5bb780a7e09f99f1200cf466ed1872ff2f8f8b3a'
$expectedZip = 'a00462b07441652d55b93e0af99d5b3c7e5c2b5a09de3322d2c13fe5a4d8df35'
$expectedExe = '3d0c6b6323165bc96543033dfe6e9c842674c5490eec7cdf4a5f726e71d0eda3'
$provenance = Get-Content (Join-Path $PackageRoot 'provenance.json') -Raw | ConvertFrom-Json
if (-not $provenance.pass -or $provenance.runId -ne '43401404329' -or $provenance.run.headSha -ne $source) {
    throw 'Diagnostic requires the accepted Build Check artifact'
}
$zip = Join-Path $PackageRoot "artifact/$($provenance.zip.name)"
$exe = Join-Path $FobDir 'fob.exe'
$before = @{ ZipSha256 = (Hash $zip); ExeSha256 = (Hash $exe) }
if ($before.ZipSha256 -ne $expectedZip -or $before.ExeSha256 -ne $expectedExe) { throw 'Accepted package hash mismatch' }
$report = [ordered]@{
    Source = $source; BuildRun = '43401404329'; AutomationSha = $env:GITHUB_SHA
    Os = [Environment]::OSVersion.VersionString; ImageOS = $env:ImageOS; ImageVersion = $env:ImageVersion
    LogicalProcessors = [Environment]::ProcessorCount; PowerShell = $PSVersionTable.PSVersion.ToString()
    Before = $before; PreflightChecks = $checks; Trials = @(); Errors = @()
    Settings = @{ behavior = @{ startInTray = $false; launchAtLogin = $false; refreshRate = 3000; language = 'en' } }
    PollMilliseconds = 250; PhaseSeconds = @(10, 30, 60)
    Normalization = 'CPU seconds = sum(kernel + user)/1e7. One-core percent = 100*CPU_s/wall_s. Machine percent divides by logical processors.'
    Readiness = 'Native visible non-minimized main window titled Fob plus owned WebViews. Does not prove DOM or data rendering.'
    Cache = 'Trial 1: fresh synthetic settings and WebView profile, not a cold OS. Trials 2-6: same warm profile after explicitly forced cleanup.'
    Limitations = 'No helper or driver installed. Unobserved short-lived processes and their CPU are unknown. Polling totals cannot attribute functions or temperature.'
}
$configDir = Join-Path $env:APPDATA 'com.hcb.fob'
$webviewDir = Join-Path $env:LOCALAPPDATA 'com.hcb.fob'
try {
    foreach ($dir in @($configDir, $webviewDir)) {
        if (Test-Path $dir) { Remove-Item -Recurse -Force -LiteralPath $dir }
    }
    [void](New-Item -ItemType Directory -Force $configDir)
    $report.Settings | ConvertTo-Json -Depth 4 | Set-Content -Encoding utf8 (Join-Path $configDir 'settings.json')
    Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue
    $report.Baseline = [FobStartup.Probe]::Baseline()
    Save 'environment.json' $report
    for ($index = 1; $index -le 6; $index++) {
        Write-Host "Starting bounded CPU trial $index of 6"
        $shot = if ($index -eq 1) { Join-Path $Evidence 'pilot-main.png' } else { $null }
        $result = [FobStartup.Probe]::Measure($exe, (Join-Path $PSScriptRoot 'win32.ps1'), $shot)
        $trial = [ordered]@{
            Index = $index; Profile = $(if ($index -eq 1) { 'fresh-profile-pilot' } else { 'warm-profile' })
            LogicalProcessors = $result.LogicalProcessors; RootPid = $result.RootPid
            LaunchApiSeconds = $result.LaunchApiSeconds; MainWindowVisibleSeconds = $result.MainWindowVisibleSeconds
            DurationSeconds = $result.DurationSeconds; MaxSampleGapSeconds = $result.MaxSampleGapSeconds
            DiscoveryWallSeconds = $result.DiscoveryWallSeconds; SamplesCount = $result.SamplesCount
            FailedCandidateCount = $result.FailedCandidateCount; RootExitedDuringMeasurement = $result.RootExitedDuringMeasurement
            RootAliveAtEnd = $result.RootAliveAtEnd; Phases = @($result.Phases); Samples = @($result.Samples)
            Processes = @($result.Processes | Select-Object Pid, Parent, Created, ExitedAt, Name, Group, Key, HelperSha256, BaselineCpu, Cpu, Total, FirstSeen, Exited, Valid, ExitCode)
            Cleanup = @($result.Cleanup); CleanupStartedSeconds = $result.CleanupStartedSeconds; Screenshot = $result.Screenshot; Errors = @($result.Errors); ShortLivedProcessGap = $result.ShortLivedProcessGap
        }
        Save "trial-$index.json" $trial
        $report.Trials += @{ Index = $index; File = "trial-$index.json"; Profile = $trial.Profile; Errors = $trial.Errors }
        Save 'environment.json' $report
        if ($result.Errors.Count -or $result.RootExitedDuringMeasurement -or -not $result.RootAliveAtEnd -or $result.Phases.Count -ne 3) {
            throw "Trial $index failed; stopping instead of reporting a fast failed launch"
        }
        if (@($result.Cleanup | Where-Object { -not $_.Gone }).Count) { throw "Trial $index cleanup incomplete" }
        if (Get-Process -Name fob -ErrorAction SilentlyContinue) { throw 'Root process remained after scoped cleanup' }
        if ((Hash $exe) -ne $expectedExe) { throw 'Executable changed during trial' }
        Start-Sleep -Seconds 2
    }
} catch {
    $report.Errors += $_.Exception.Message
    throw
} finally {
    $report.After = @{ ZipSha256 = (Hash $zip); ExeSha256 = (Hash $exe) }
    $report.HashesUnchanged = $report.After.ZipSha256 -eq $expectedZip -and $report.After.ExeSha256 -eq $expectedExe
    Save 'environment.json' $report
    if (-not $report.HashesUnchanged) { throw 'Accepted package changed' }
}
Write-Host 'Six bounded CPU trials completed; cleanup was forced only after each measurement'
