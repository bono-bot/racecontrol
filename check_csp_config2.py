"""Check CSP telemetry/UDP config on a pod via PowerShell."""
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

print(f"=== CSP Telemetry Config on {pod_ip} ===\n")

# Read general.ini for UDP-related settings
result = ps_exec(r"""
$base = 'C:\Program Files (x86)\Steam\steamapps\common\assettocorsa\extension\config'
$files = @('general.ini', 'gui.ini', 'small_tweaks.ini', 'new_behaviour.ini')
foreach ($f in $files) {
    $path = Join-Path $base $f
    if (Test-Path $path) {
        $content = Get-Content $path -Raw
        $lines = $content -split "`n"
        $relevant = @()
        $inSection = $false
        foreach ($line in $lines) {
            if ($line -match '^\[') {
                if ($line -match 'UDP|TELEMETRY|OUTPUT|BROADCAST|DATA|REMOTE|NETWORK|CODEMASTERS') {
                    $inSection = $true
                    $relevant += ""
                    $relevant += $line
                } else {
                    $inSection = $false
                }
            } elseif ($inSection) {
                $relevant += $line
            }
        }
        if ($relevant.Count -gt 0) {
            Write-Output "=== $f ==="
            $relevant | ForEach-Object { Write-Output $_ }
        }
    }
}

# Also check if there's a standalone data output config or telemetry setting
$content = Get-Content (Join-Path $base 'general.ini') -Raw
if ($content -match 'ENABLED_CARS_DATA') {
    Write-Output ""
    Write-Output "=== general.ini CARS_DATA section ==="
    # Get the section
    $match = [regex]::Match($content, '\[ENABLED_CARS_DATA\][^\[]*')
    if ($match.Success) {
        Write-Output $match.Value.Trim()
    }
}
""")
print(result.get('stdout', '') or 'No relevant sections found')
if result.get('stderr') and 'Preparing modules' not in result.get('stderr',''):
    print("STDERR:", result.get('stderr')[:300])

# Now specifically check for Codemasters/F1 UDP output setting
print("\n--- Searching for UDP output settings ---")
result2 = ps_exec(r"""
$base = 'C:\Program Files (x86)\Steam\steamapps\common\assettocorsa\extension\config'
Get-ChildItem $base -Filter '*.ini' | ForEach-Object {
    $content = Get-Content $_.FullName -Raw
    if ($content -match 'UDP|CODEMASTERS|20777|9996|BROADCAST') {
        Write-Output "FILE: $($_.Name)"
        $lines = $content -split "`n"
        foreach ($line in $lines) {
            if ($line -match 'UDP|CODEMASTERS|20777|9996|BROADCAST|PORT') {
                Write-Output "  $($line.Trim())"
            }
        }
    }
}
""")
print(result2.get('stdout', '') or 'No UDP config found in CSP')
