# Native window facts and captures for run.mjs. Prints one JSON value.
# Runs in Windows PowerShell 5.1, whose .NET Framework has GDI+ built in.
param([Parameter(Mandatory)][string]$Action, [string]$Arg, [string]$Out)
$ErrorActionPreference = "Stop"
Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @"
using System;
using System.Collections.Generic;
using System.Drawing;
using System.Runtime.InteropServices;
using System.Text;
public static class W {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
  public static List<IntPtr> Of(uint pid) {
    var all = new List<IntPtr>();
    EnumWindows((h, l) => { uint p; GetWindowThreadProcessId(h, out p); if (p == pid) all.Add(h); return true; }, IntPtr.Zero);
    return all;
  }
  public static string Text(IntPtr h) { var s = new StringBuilder(256); GetWindowText(h, s, 256); return s.ToString(); }
  public static string Class(IntPtr h) { var s = new StringBuilder(256); GetClassName(h, s, 256); return s.ToString(); }
  public static Bitmap Print(IntPtr h) {
    RECT r; GetWindowRect(h, out r);
    var b = new Bitmap(Math.Max(1, r.R - r.L), Math.Max(1, r.B - r.T));
    using (var g = Graphics.FromImage(b)) { var dc = g.GetHdc(); PrintWindow(h, dc, 2); g.ReleaseHdc(dc); }
    return b;
  }
}
"@
function Window($h) {
  $r = New-Object W+RECT; [void][W]::GetWindowRect($h, [ref]$r)
  [ordered]@{ hwnd = [int64]$h; title = [W]::Text($h); class = [W]::Class($h)
    visible = [W]::IsWindowVisible($h); iconic = [W]::IsIconic($h)
    foreground = ([W]::GetForegroundWindow() -eq $h); dpi = [W]::GetDpiForWindow($h)
    rect = @($r.L, $r.T, ($r.R - $r.L), ($r.B - $r.T)) }
}
switch ($Action) {
  "windows" { ConvertTo-Json -InputObject @([W]::Of([uint32]$Arg) | ForEach-Object { Window $_ }) -Depth 4 -Compress }
  "client-origin" { $p = New-Object W+POINT; [void][W]::ClientToScreen([IntPtr][int64]$Arg, [ref]$p); @{ x = $p.X; y = $p.Y } | ConvertTo-Json -Compress }
  "cursor" { $x, $y = $Arg -split ","; [W]::SetCursorPos([int]$x, [int]$y) | ConvertTo-Json }
  "close" { [W]::PostMessage([IntPtr][int64]$Arg, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero) | ConvertTo-Json }
  "window-shot" { $b = [W]::Print([IntPtr][int64]$Arg); $b.Save($Out); @{ width = $b.Width; height = $b.Height } | ConvertTo-Json -Compress }
  "screen-shot" {
    Add-Type -AssemblyName System.Windows.Forms
    $s = [System.Windows.Forms.SystemInformation]::VirtualScreen
    $b = New-Object System.Drawing.Bitmap $s.Width, $s.Height
    $g = [System.Drawing.Graphics]::FromImage($b); $g.CopyFromScreen($s.Left, $s.Top, 0, 0, $b.Size); $g.Dispose()
    $b.Save($Out); @{ width = $s.Width; height = $s.Height } | ConvertTo-Json -Compress
  }
  "elevated" {
    $t = [System.Security.Principal.WindowsIdentity]::GetCurrent()
    $admin = (New-Object System.Security.Principal.WindowsPrincipal $t).IsInRole([System.Security.Principal.WindowsBuiltInRole]::Administrator)
    $uac = (Get-ItemProperty HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System).EnableLUA
    @{ runnerIsAdmin = $admin; enableLUA = $uac; fobPath = (Get-Process -Id ([int]$Arg)).Path } | ConvertTo-Json -Compress
  }
  "webview-processes" {
    @(Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" | Where-Object { $_.CommandLine -like "*$Arg*" }).Count | ConvertTo-Json
  }
  default { throw "unknown action $Action" }
}
