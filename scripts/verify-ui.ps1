[CmdletBinding()]
param(
    [string]$Exe,
    [switch]$Force,
    [switch]$KeepOpen
)

$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding $false

function Stop-Verify([string]$Message, [int]$Code) {
    Write-Error $Message -ErrorAction Continue
    exit $Code
}

if (-not [Environment]::OSVersion.Platform.ToString().StartsWith('Win')) {
    Stop-Verify 'verify:ui is supported only on Windows.' 3
}

if (-not $Exe) {
    $Exe = Get-ChildItem -Path (Join-Path $PSScriptRoot '..\dist') -Filter 'fob.exe' -File -Recurse -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTimeUtc -Descending |
        Select-Object -First 1 -ExpandProperty FullName
    if (-not $Exe) {
        Stop-Verify 'Could not find fob.exe under dist/. Pass -Exe <path> to the Windows executable.' 2
    }
}

try {
    $Exe = (Resolve-Path -LiteralPath $Exe).Path
} catch {
    Stop-Verify "Executable not found: $Exe" 2
}
if ([IO.Path]::GetFileName($Exe) -ne 'fob.exe') {
    Stop-Verify "Expected fob.exe, got: $Exe" 2
}

$existing = @(Get-Process -Name 'fob' -ErrorAction SilentlyContinue)
if ($existing.Count -gt 0) {
    if (-not $Force) {
        $pids = ($existing | ForEach-Object { $_.Id }) -join ', '
        Stop-Verify "A fob instance is already running (PID $pids). Refusing to stop it; close it yourself or rerun with -Force." 1
    }
    $pids = ($existing | ForEach-Object { $_.Id }) -join ', '
    $answer = Read-Host "-Force will terminate existing fob process(es) PID $pids. Type KILL to continue"
    if ($answer -cne 'KILL') {
        Stop-Verify 'Confirmation was not KILL; no process was stopped.' 1
    }
    foreach ($process in $existing) {
        Write-Output "Stopping existing fob PID $($process.Id) (-Force confirmed)."
        Stop-Process -Id $process.Id -Force
    }
}

$webViewRuntime = $false
$runtimeKeyPaths = @(
    'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}',
    'HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}',
    'HKCU:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
)
foreach ($path in $runtimeKeyPaths) {
    if (Test-Path $path) { $webViewRuntime = $true; break }
}
if (-not $webViewRuntime) {
    $runtimeDirs = @(
        (Join-Path $env:ProgramFiles '(x86)\Microsoft\EdgeWebView\Application'),
        (Join-Path $env:ProgramFiles 'Microsoft\EdgeWebView\Application'),
        (Join-Path $env:LOCALAPPDATA 'Microsoft\EdgeWebView\Application')
    )
    foreach ($dir in $runtimeDirs) {
        if (Test-Path $dir) { $webViewRuntime = $true; break }
    }
}
if (-not $webViewRuntime) {
    Stop-Verify 'Microsoft Edge WebView2 Runtime was not found.' 3
}

try {
    Add-Type -AssemblyName UIAutomationClient
    Add-Type -AssemblyName UIAutomationTypes
} catch {
    Stop-Verify "Windows UI Automation is unavailable: $($_.Exception.Message)" 3
}

$started = $null
$exitCode = 0
$checksPrinted = $false
$checks = [ordered]@{
    'Accessibility tree has more than 100 elements' = $false
    'Dashboard, process and space tabs exist' = $false
    'Process table has a non-header DataItem row' = $false
    'Status text contains 实时 · N 秒' = $false
    'At least one numeric or percentage Text value exists' = $false
    'Fob status bar window exists' = $false
}
try {
    $oldBrowserArgs = [Environment]::GetEnvironmentVariable('WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS', 'Process')
    [Environment]::SetEnvironmentVariable('WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS', '--force-renderer-accessibility', 'Process')
    try {
        $started = Start-Process -FilePath $Exe -ArgumentList '--show-all' -PassThru
    } finally {
        [Environment]::SetEnvironmentVariable('WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS', $oldBrowserArgs, 'Process')
    }

    $window = $null
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    while ([DateTime]::UtcNow -lt $deadline) {
        try {
            $windows = [System.Windows.Automation.AutomationElement]::RootElement.FindAll(
                [System.Windows.Automation.TreeScope]::Children,
                [System.Windows.Automation.Condition]::TrueCondition
            )
            foreach ($candidate in $windows) {
                if ($candidate.Current.ProcessId -eq $started.Id -and $candidate.Current.ClassName -eq 'Tauri Window' -and $candidate.Current.Name -eq 'Fob') {
                    $window = $candidate
                    break
                }
            }
        } catch { }
        if ($window) { break }
        Start-Sleep -Milliseconds 500
    }

    if (-not $window) { throw 'Could not find the launched Fob main window within 30 seconds despite --show-all. Check the app startup logs and WebView2 runtime.' }

    $statusBar = $null
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    while ([DateTime]::UtcNow -lt $deadline) {
        try {
            $windows = [System.Windows.Automation.AutomationElement]::RootElement.FindAll(
                [System.Windows.Automation.TreeScope]::Children,
                [System.Windows.Automation.Condition]::TrueCondition
            )
            foreach ($candidate in $windows) {
                if ($candidate.Current.ProcessId -eq $started.Id -and $candidate.Current.Name -eq 'Fob status bar') {
                    $statusBar = $candidate
                    break
                }
            }
        } catch { }
        if ($statusBar) { break }
        Start-Sleep -Milliseconds 500
    }
    $checks['Fob status bar window exists'] = $null -ne $statusBar

    $descendants = $window.FindAll(
        [System.Windows.Automation.TreeScope]::Descendants,
        [System.Windows.Automation.Condition]::TrueCondition
    )
    $elements = @($descendants)
    $checks['Accessibility tree has more than 100 elements'] = $elements.Count -gt 100

    $tabs = @('仪表盘', '进程', '空间分布')
    $tabElements = @{}
    foreach ($tabName in $tabs) {
        $match = $null
        foreach ($element in $elements) {
            if ($element.Current.ControlType -eq [System.Windows.Automation.ControlType]::Button -and $element.Current.Name -eq $tabName) {
                $match = $element
                break
            }
        }
        $tabElements[$tabName] = $match
    }
    $checks['Dashboard, process and space tabs exist'] = @($tabs | Where-Object { -not $tabElements[$_] }).Count -eq 0

    $processTab = $tabElements['进程']
    if (-not $processTab) { throw 'Cannot switch tabs: button named 进程 was not found.' }
    $switched = $false
    $switchErrors = [System.Collections.Generic.List[string]]::new()
    $patternOptions = @(
        @{ Name = 'InvokePattern'; Pattern = [System.Windows.Automation.InvokePattern]::Pattern },
        @{ Name = 'TogglePattern'; Pattern = [System.Windows.Automation.TogglePattern]::Pattern },
        @{ Name = 'SelectionItemPattern'; Pattern = [System.Windows.Automation.SelectionItemPattern]::Pattern }
    )
    foreach ($option in $patternOptions) {
        try {
            $pattern = $null
            if ($processTab.TryGetCurrentPattern($option.Pattern, [ref]$pattern)) {
                switch ($option.Name) {
                    'InvokePattern' { $pattern.Invoke() }
                    'TogglePattern' { $pattern.Toggle() }
                    'SelectionItemPattern' { $pattern.Select() }
                }
                $switched = $true
                break
            }
        } catch { $switchErrors.Add("$($option.Name): $($_.Exception.Message)") }
    }
    if (-not $switched) {
        $detail = if ($switchErrors.Count) { $switchErrors -join '; ' } else { 'no supported Invoke, Toggle or SelectionItem pattern' }
        throw "Cannot switch to the process tab: $detail"
    }
    Start-Sleep -Seconds 2
    $descendants = $window.FindAll(
        [System.Windows.Automation.TreeScope]::Descendants,
        [System.Windows.Automation.Condition]::TrueCondition
    )
    $elements = @($descendants)
    $dataRows = @($elements | Where-Object {
        $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::DataItem -and
        $_.Current.Name -and $_.Current.Name -notmatch '^(进程名称|状态|PID|CPU\s*%|内存)$'
    })
    $checks['Process table has a non-header DataItem row'] = $dataRows.Count -gt 0

    $texts = @($elements | Where-Object { $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Text } | ForEach-Object { $_.Current.Name })
    $checks['Status text contains 实时 · N 秒'] = @($texts | Where-Object { $_ -match '实时\s*[·•]\s*\d+\s*秒' }).Count -gt 0
    $checks['At least one numeric or percentage Text value exists'] = @($texts | Where-Object { $_ -match '^\s*(?:\d+(?:\.\d+)?%?|\d+(?:\.\d+)?\s*%)\s*$' }).Count -gt 0

    foreach ($check in $checks.GetEnumerator()) {
        if ($check.Value) { Write-Output "PASS: $($check.Key)" }
        else { Write-Output "FAIL: $($check.Key)"; $exitCode = 1 }
    }
    $checksPrinted = $true
} catch {
    Write-Output "ERROR: $($_.Exception.Message)"
    $exitCode = 1
} finally {
    if (-not $checksPrinted) {
        foreach ($check in $checks.GetEnumerator()) {
            if ($check.Value) { Write-Output "PASS: $($check.Key)" }
            else { Write-Output "FAIL: $($check.Key)" }
        }
    }
    if ($started -and -not $KeepOpen) {
        $running = Get-Process -Id $started.Id -ErrorAction SilentlyContinue
        if ($running) {
            Stop-Process -Id $started.Id -Force
            Write-Output "Closed verification process PID $($started.Id)."
        }
    } elseif ($started -and $KeepOpen) {
        Write-Output "Keeping verification process PID $($started.Id) open (-KeepOpen)."
    }
}
exit $exitCode
