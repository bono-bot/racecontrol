param(
    [string]$OutFile = ""
)

$ErrorActionPreference = 'SilentlyContinue'

$result = [ordered]@{
    host             = $env:COMPUTERNAME
    user             = $env:USERNAME
    timestamp_utc    = (Get-Date).ToUniversalTime().ToString("o")
    last_boot_utc    = ((Get-CimInstance Win32_OperatingSystem).LastBootUpTime).ToUniversalTime().ToString("o")
    surfaces         = [ordered]@{}
}

function Read-RunKey($path) {
    $p = Get-ItemProperty -Path $path -ErrorAction SilentlyContinue
    if (-not $p) { return @() }
    $p.PSObject.Properties |
        Where-Object { $_.Name -notmatch '^PS' } |
        ForEach-Object { @{ name = $_.Name; value = "$($_.Value)" } }
}

$result.surfaces["hklm_run"]    = Read-RunKey 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run'
$result.surfaces["hklm_runonce"]= Read-RunKey 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce'
$result.surfaces["hkcu_run"]    = Read-RunKey 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run'
$result.surfaces["hkcu_runonce"]= Read-RunKey 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce'

function Read-StartupDir($dir) {
    if (-not (Test-Path $dir)) { return @() }
    Get-ChildItem $dir -ErrorAction SilentlyContinue | ForEach-Object {
        $entry = @{ name = $_.Name; path = $_.FullName }
        if ($_.Extension -eq '.lnk') {
            try {
                $shell = New-Object -ComObject WScript.Shell
                $sc = $shell.CreateShortcut($_.FullName)
                $entry.target    = $sc.TargetPath
                $entry.arguments = $sc.Arguments
            } catch {}
        }
        $entry
    }
}

$result.surfaces["startup_all_users"] = Read-StartupDir "$env:ProgramData\Microsoft\Windows\Start Menu\Programs\StartUp"
$result.surfaces["startup_user"]      = Read-StartupDir "$env:USERPROFILE\AppData\Roaming\Microsoft\Windows\Start Menu\Programs\Startup"

$tasks = schtasks /Query /FO CSV /V 2>$null | ConvertFrom-Csv
$result.surfaces["scheduled_tasks"] = $tasks |
    Where-Object { $_."Task To Run" -and $_."Task To Run" -ne "COM handler" -and $_."TaskName" -and $_."TaskName" -notmatch '^\\Microsoft\\Windows\\' } |
    ForEach-Object {
        @{
            name         = $_."TaskName"
            command      = $_."Task To Run"
            run_as       = $_."Run As User"
            last_run     = $_."Last Run Time"
            last_result  = $_."Last Result"
            status       = $_.Status
        }
    }

$services = Get-CimInstance Win32_Service -ErrorAction SilentlyContinue |
    Where-Object { $_.StartMode -eq 'Auto' -and $_.PathName -notmatch '^"?C:\\Windows\\system32' }
$result.surfaces["auto_services"] = $services | ForEach-Object {
    @{ name = $_.Name; display = $_.DisplayName; path = $_.PathName; state = $_.State; start = $_.StartMode }
}

$racingPointRoot = 'C:\RacingPoint'
$browserRefs = @()
if (Test-Path $racingPointRoot) {
    Get-ChildItem $racingPointRoot -Include *.ps1,*.bat,*.cmd,*.reg,*.xml -File -Recurse -Depth 2 -ErrorAction SilentlyContinue |
        ForEach-Object {
            $content = Get-Content $_.FullName -Raw -ErrorAction SilentlyContinue
            if ($content -and ($content -match 'msedge\.exe|chrome\.exe|--kiosk|--edge-kiosk-type|edge-kiosk-type=fullscreen|remote-debugging-port')) {
                $browserRefs += @{
                    path = $_.FullName
                    size = $_.Length
                    mtime = $_.LastWriteTime.ToString("o")
                    matches = ([regex]::Matches($content, '(?i)(msedge\.exe|chrome\.exe|--kiosk|--edge-kiosk-type|remote-debugging-port)') |
                               ForEach-Object { $_.Value } | Select-Object -Unique) -join ','
                }
            }
        }
}
$result.surfaces["on_disk_browser_refs"] = $browserRefs

$procs = Get-CimInstance Win32_Process -Filter "Name='chrome.exe' OR Name='msedge.exe'" -ErrorAction SilentlyContinue
$result.surfaces["running_browsers"] = $procs | ForEach-Object {
    @{
        name    = $_.Name
        pid     = $_.ProcessId
        ppid    = $_.ParentProcessId
        started = $_.CreationDate.ToString("o")
        cmd     = "$($_.CommandLine)".Substring(0, [Math]::Min(300, "$($_.CommandLine)".Length))
    }
}

$json = $result | ConvertTo-Json -Depth 10

if ($OutFile) {
    $json | Out-File -FilePath $OutFile -Encoding UTF8
    Write-Host ("Wrote " + $OutFile)
} else {
    Write-Output $json
}
