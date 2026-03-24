Add-Type -AssemblyName System.Windows.Forms
$screens = [System.Windows.Forms.Screen]::AllScreens
foreach($s in $screens) {
    Write-Output "Screen: $($s.DeviceName) Primary=$($s.Primary) Bounds=$($s.Bounds)"
}
$virt = [System.Windows.Forms.SystemInformation]::VirtualScreen
Write-Output "VirtualScreen=$virt"

# Check Edge windows
Get-Process msedge -ErrorAction SilentlyContinue | Select-Object Id, MainWindowTitle | Format-Table -AutoSize
