Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public class TB {
    [DllImport("user32.dll")] public static extern IntPtr FindWindow(string c, string t);
    [DllImport("user32.dll")] public static extern int ShowWindow(IntPtr h, int n);
}
"@
$taskbar = [TB]::FindWindow("Shell_TrayWnd", $null)
if ($taskbar -ne [IntPtr]::Zero) {
    [TB]::ShowWindow($taskbar, 0)
    Write-Output "TASKBAR HIDDEN"
} else {
    Write-Output "TASKBAR NOT FOUND"
}
$start = [TB]::FindWindow("Button", "Start")
if ($start -ne [IntPtr]::Zero) { [TB]::ShowWindow($start, 0) }
