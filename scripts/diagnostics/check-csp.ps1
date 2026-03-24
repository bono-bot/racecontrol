$acDir = "C:\Program Files (x86)\Steam\steamapps\common\assettocorsa"
$ext = "$acDir\extension"

Write-Host "=== CSP version files ==="
Get-ChildItem "$ext\config" -File -ErrorAction SilentlyContinue | Select-Object Name, Length | Format-Table

Write-Host "=== CSP data_manifest.ini (force_start section) ==="
$manifest = "$ext\config\data_manifest.ini"
if (Test-Path $manifest) {
    Select-String -Path $manifest -Pattern "force_start|FORCE_START|hide_main" -Context 2 -ErrorAction SilentlyContinue
}

Write-Host "=== CSP general.ini ==="
$general = "$ext\config\general.ini"  
if (Test-Path $general) {
    Get-Content $general -TotalCount 30
} else {
    Write-Host "general.ini not found"
}

Write-Host "=== gui.ini from CSP extension ==="
Get-ChildItem "$ext" -Filter "gui*" -Recurse -ErrorAction SilentlyContinue | Select-Object FullName

Write-Host "=== Search for FORCE_START in CSP configs ==="
Get-ChildItem "$ext\config" -Filter "*.ini" -Recurse -ErrorAction SilentlyContinue | Select-String "FORCE_START" -ErrorAction SilentlyContinue
