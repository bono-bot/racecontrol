Write-Host "=== Steam process ==="
$steam = Get-Process steam -ErrorAction SilentlyContinue
if ($steam) { Write-Host "Steam is RUNNING (PID: $($steam.Id))" } else { Write-Host "Steam is NOT RUNNING" }

Write-Host "`n=== AC process ==="
$ac = Get-Process acs -ErrorAction SilentlyContinue
if ($ac) { Write-Host "AC is RUNNING (PID: $($ac.Id))" } else { Write-Host "AC is NOT running" }

Write-Host "`n=== Logged-in sessions ==="
query user 2>$null

Write-Host "`n=== AC steam_appid.txt ==="
$acDir = "C:\Program Files (x86)\Steam\steamapps\common\assettocorsa"
if (Test-Path "$acDir\steam_appid.txt") {
    Get-Content "$acDir\steam_appid.txt"
} else {
    Write-Host "steam_appid.txt NOT FOUND"
}

Write-Host "`n=== Display info ==="
Get-CimInstance Win32_VideoController | Select-Object Name, DriverVersion, Status | Format-Table
