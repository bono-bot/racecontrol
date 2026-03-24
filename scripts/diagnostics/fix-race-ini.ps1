$raceIni = "$env:USERPROFILE\Documents\Assetto Corsa\cfg\race.ini"
$content = Get-Content $raceIni -Raw
# Replace SKIN=00_default with SKIN= (empty)
$content = $content -replace 'SKIN=00_default', 'SKIN='
Set-Content -Path $raceIni -Value $content -Encoding ASCII -NoNewline
Write-Host "Fixed race.ini - SKIN= (empty)"
# Show the CAR_0 and RACE sections
Select-String -Path $raceIni -Pattern "SKIN=" | ForEach-Object { Write-Host $_.Line }
