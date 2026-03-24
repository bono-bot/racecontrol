# fix-blanking.ps1 — Kill overlay processes + hide taskbar + enforce kiosk foreground
# Designed to run via rc-agent exec on each pod

Write-Output "=== Killing overlay processes ==="
$killList = @(
    "NVIDIA Overlay", "nvsphelper64", "amdow", "AMDRSServ", "AMDRSSrcExt",
    "Copilot", "M365Copilot", "OpenWith", "Notepad", "VNMConfig",
    "SystemSettings", "WindowsTerminal", "Widgets", "PhoneExperienceHost",
    "OneDrive", "SearchHost", "StartMenuExperienceHost"
)
foreach ($name in $killList) {
    $procs = Get-Process -Name $name -ErrorAction SilentlyContinue
    if ($procs) {
        $procs | Stop-Process -Force -ErrorAction SilentlyContinue
        Write-Output "  Killed: $name ($($procs.Count) instances)"
    }
}

Write-Output ""
Write-Output "=== Hiding taskbar ==="
# Method 1: Set auto-hide via registry
$regPath = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\StuckRects3"
$settings = (Get-ItemProperty -Path $regPath -Name Settings).Settings
$settings[8] = $settings[8] -bor 0x01  # Enable auto-hide bit
Set-ItemProperty -Path $regPath -Name Settings -Value $settings

# Method 2: Group Policy — hide taskbar entirely
$polPath = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer"
if (-not (Test-Path $polPath)) { New-Item -Path $polPath -Force | Out-Null }
Set-ItemProperty -Path $polPath -Name "NoTaskbar" -Value 0 -Type DWord -ErrorAction SilentlyContinue

# Method 3: Hide taskbar window directly using Win32 API
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public class TaskbarHider {
    [DllImport("user32.dll")] public static extern IntPtr FindWindow(string className, string windowName);
    [DllImport("user32.dll")] public static extern int ShowWindow(IntPtr hwnd, int nCmdShow);
    public const int SW_HIDE = 0;
    public const int SW_SHOW = 5;
    public static void Hide() {
        IntPtr taskbar = FindWindow("Shell_TrayWnd", null);
        if (taskbar != IntPtr.Zero) ShowWindow(taskbar, SW_HIDE);
        IntPtr startBtn = FindWindow("Button", "Start");
        if (startBtn != IntPtr.Zero) ShowWindow(startBtn, SW_HIDE);
    }
}
"@ -ErrorAction SilentlyContinue
[TaskbarHider]::Hide()
Write-Output "  Taskbar hidden via FindWindow + ShowWindow(SW_HIDE)"

Write-Output ""
Write-Output "=== Enforcing kiosk foreground ==="
# Find the Edge kiosk window and force it to foreground + maximize
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
using System.Text;
public class KioskFG {
    [DllImport("user32.dll")] public static extern IntPtr FindWindow(string c, string t);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int n);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc proc, IntPtr param);
    [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    public delegate bool EnumWindowsProc(IntPtr h, IntPtr p);
    public const int SW_MAXIMIZE = 3;
}
"@ -ErrorAction SilentlyContinue

$found = $false
[KioskFG]::EnumWindows({
    param($hwnd, $param)
    $sb = New-Object System.Text.StringBuilder 256
    [KioskFG]::GetWindowText($hwnd, $sb, 256)
    $title = $sb.ToString()
    if ($title -match "Racing Point" -or $title -match "RacingPoint") {
        [KioskFG]::SetForegroundWindow($hwnd)
        [KioskFG]::ShowWindow($hwnd, [KioskFG]::SW_MAXIMIZE)
        Write-Output "  Foreground set: $title"
        $script:found = $true
    }
    return $true
}, [IntPtr]::Zero)

if (-not $found) { Write-Output "  WARNING: No Racing Point kiosk window found" }

Write-Output ""
Write-Output "DONE"
