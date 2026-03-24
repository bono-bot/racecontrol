Write-Output "=== Monitors ==="
Add-Type -AssemblyName System.Windows.Forms
[System.Windows.Forms.Screen]::AllScreens | ForEach-Object {
    Write-Output "  $($_.DeviceName): $($_.Bounds.Width)x$($_.Bounds.Height) Primary=$($_.Primary)"
}
Write-Output ""
Write-Output "=== Virtual screen (all monitors combined) ==="
Write-Output "  $([System.Windows.Forms.SystemInformation]::VirtualScreen)"

Write-Output ""
Write-Output "=== Foreground window ==="
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
using System.Text;
public class FG {
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out int[] r);
}
"@ -ErrorAction SilentlyContinue
$hw = [FG]::GetForegroundWindow()
$sb = New-Object System.Text.StringBuilder 256
[FG]::GetWindowText($hw, $sb, 256) | Out-Null
Write-Output "  Foreground: $($sb.ToString())"

Write-Output ""
Write-Output "=== Taskbar state ==="
$settings = (Get-ItemProperty "HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\StuckRects3" -Name Settings -ErrorAction SilentlyContinue).Settings
if ($settings) {
    $byte8 = $settings[8]
    Write-Output "  Byte8=$byte8 AutoHide=$($byte8 -band 1)"
}

Write-Output ""
Write-Output "=== Notification count ==="
try {
    $notifs = Get-Process -Name "ShellExperienceHost" -ErrorAction SilentlyContinue
    if ($notifs) { Write-Output "  ShellExperienceHost running (may show notifications)" }
    $toast = Get-Process -Name "Windows.UI.Notifications*" -ErrorAction SilentlyContinue
    if ($toast) { Write-Output "  Toast notification process found" }
} catch {}
Write-Output "DONE"
