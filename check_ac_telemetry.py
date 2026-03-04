"""Check AC telemetry sources on a pod."""
import json, urllib.request, base64, sys

pod_ip = sys.argv[1] if len(sys.argv) > 1 else "192.168.31.91"

def ps_exec(script, timeout=30):
    encoded = base64.b64encode(script.encode('utf-16-le')).decode('ascii')
    cmd = f"cd /d C:\\RacingPoint & powershell -NoProfile -EncodedCommand {encoded}"
    data = json.dumps({"cmd": cmd}).encode()
    req = urllib.request.Request(
        f"http://{pod_ip}:8090/exec",
        data=data,
        headers={"Content-Type": "application/json"}
    )
    resp = urllib.request.urlopen(req, timeout=timeout)
    return json.loads(resp.read().decode())

print(f"=== AC Telemetry Investigation on {pod_ip} ===\n")

# 1. Check AC Python apps
print("--- AC Python apps ---")
result = ps_exec(r"""
$appsPath = 'C:\Program Files (x86)\Steam\steamapps\common\assettocorsa\apps\python'
if (Test-Path $appsPath) {
    Get-ChildItem $appsPath -Directory | ForEach-Object { Write-Output $_.Name }
} else {
    Write-Output "No Python apps directory"
}
""")
print(result.get('stdout', '') or 'None')

# 2. Check enabled apps in race.ini Python section
print("\n--- Enabled AC apps ---")
result = ps_exec(r"""
$acLog = 'C:\Users\User\Documents\Assetto Corsa\logs\log.txt'
if (Test-Path $acLog) {
    $content = Get-Content $acLog -Tail 100
    $content | Select-String 'app|python|udp|telemetry|shared|memory|port' -CaseSensitive:$false | Select-Object -First 20
}
""")
print(result.get('stdout', '') or 'No relevant log entries')

# 3. Check Conspit Link config / settings
print("\n--- Conspit Link directory ---")
result = ps_exec(r"""
Get-ChildItem 'C:\Program Files (x86)\Conspit Link 2.0' -Recurse -Include '*.ini','*.cfg','*.json','*.xml','*.config' | ForEach-Object {
    Write-Output $_.FullName
}
""")
print(result.get('stdout', '') or 'No config files')

# 4. Check AC race.ini for any UDP settings
print("\n--- race.ini relevant sections ---")
result = ps_exec(r"""
$raceIni = Get-Content 'C:\Users\User\Documents\Assetto Corsa\cfg\race.ini' -Raw
$lines = $raceIni -split "`n"
foreach ($line in $lines) {
    if ($line -match 'UDP|PORT|TELEMETRY|REMOTE|BROADCAST') {
        Write-Output $line.Trim()
    }
}
""")
print(result.get('stdout', '') or 'No UDP settings in race.ini')

# 5. Check AC cfg directory for any telemetry/UDP related files
print("\n--- AC cfg files ---")
result = ps_exec(r"""
Get-ChildItem 'C:\Users\User\Documents\Assetto Corsa\cfg' -Name
""")
print(result.get('stdout', '') or 'No files')

# 6. Check if shared memory exists
print("\n--- AC Shared Memory ---")
result = ps_exec(r"""
$sharedMem = @(
    'acpmf_physics', 'acpmf_graphics', 'acpmf_static',
    'Local\acpmf_physics', 'Local\acpmf_graphics', 'Local\acpmf_static'
)
[System.IO.MemoryMappedFiles.MemoryMappedFile] | Out-Null
foreach ($name in $sharedMem) {
    try {
        $mmf = [System.IO.MemoryMappedFiles.MemoryMappedFile]::OpenExisting($name)
        Write-Output "FOUND: $name"
        $mmf.Dispose()
    } catch {
        # not found
    }
}
""")
print(result.get('stdout', '') or 'No shared memory found')
