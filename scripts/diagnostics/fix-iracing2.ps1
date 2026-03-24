$path = "C:\RacingPoint\rc-agent.toml"
$lines = Get-Content $path
$newLines = @()
$skip = $false

foreach ($line in $lines) {
    if ($line -match '^\[games\.iracing\]') {
        # Replace the entire iracing section
        $newLines += '[games.iracing]'
        $newLines += 'steam_app_id = 266410'
        $newLines += 'use_steam = true'
        $skip = $true
        continue
    }
    if ($skip) {
        # Skip old iracing lines until next section or blank line after values
        if ($line -match '^\[' -or ($line.Trim() -eq '' -and $newLines[-1].Trim() -ne '')) {
            $skip = $false
            $newLines += $line
        }
        continue
    }
    $newLines += $line
}

$newLines | Set-Content $path -Encoding UTF8
$check = Select-String -Path $path -Pattern '266410'
if ($check) { Write-Output "OK: iRacing fixed to Steam 266410" }
else { Write-Output "FAIL" }
