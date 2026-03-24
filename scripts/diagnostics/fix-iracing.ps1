$path = "C:\RacingPoint\rc-agent.toml"
$content = Get-Content $path -Raw

# Replace iRacing config: switch from direct exe to Steam launch
$old = '[games.iracing]
exe_path = "C:\\Program Files (x86)\\iRacing\\iRacingSim64DX11.exe"
use_steam = false'

$new = '[games.iracing]
steam_app_id = 266410
use_steam = true'

if ($content -match 'exe_path.*iRacing') {
    $content = $content -replace [regex]::Escape($old), $new
    [IO.File]::WriteAllText($path, $content)
    $check = Select-String -Path $path -Pattern '266410'
    if ($check) { Write-Output "OK: iRacing switched to Steam launch (266410)" }
    else { Write-Output "FAIL: replacement didn't stick" }
} else {
    Write-Output "SKIP: iRacing config not in expected format"
}
