$acDir = "C:\Program Files (x86)\Steam\steamapps\common\assettocorsa"
$docsDir = "$env:USERPROFILE\Documents\Assetto Corsa"

Write-Host "=== AC crash.log ==="
if (Test-Path "$docsDir\logs\crash.log") {
    Get-Content "$docsDir\logs\crash.log" -Tail 30
} else {
    Write-Host "No crash.log found"
    # Check for any log files
    if (Test-Path "$docsDir\logs") {
        Write-Host "Log files in docs:"
        Get-ChildItem "$docsDir\logs" -File | Select-Object Name, LastWriteTime | Format-Table
    }
}

Write-Host "`n=== AC log.txt ==="
if (Test-Path "$docsDir\logs\log.txt") {
    Get-Content "$docsDir\logs\log.txt" -Tail 30
} elseif (Test-Path "$acDir\log.txt") {
    Get-Content "$acDir\log.txt" -Tail 30
} else {
    Write-Host "No log.txt found"
}

Write-Host "`n=== Windows Event Log (recent app crashes) ==="
Get-WinEvent -FilterHashtable @{LogName='Application'; Level=2; StartTime=(Get-Date).AddHours(-1)} -MaxEvents 5 -ErrorAction SilentlyContinue | Select-Object TimeCreated, Message | Format-List
