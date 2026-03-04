"""
Restart Conspit Link 2.0 on a pod after AC is running.
Conspit Link needs to re-handshake with AC's UDP telemetry (port 9996).
"""
import json, urllib.request, base64, sys, time

pod_ip = sys.argv[1] if len(sys.argv) > 1 else "192.168.31.91"

def ps_exec(pod_ip, script, timeout=30):
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

# Check current state
print(f"=== Conspit Link fix on {pod_ip} ===")
print()

result = ps_exec(pod_ip, """
$acs = Get-Process -Name acs -ErrorAction SilentlyContinue
$cl = Get-Process -Name ConspitLink2.0 -ErrorAction SilentlyContinue
Write-Output "AC running: $($acs -ne $null)"
Write-Output "Conspit Link running: $($cl -ne $null)"
if ($cl) { Write-Output "Conspit PID: $($cl.Id)" }
""")
print(result.get("stdout", "").strip())
print()

# Restart Conspit Link
print("Restarting Conspit Link 2.0...")
result = ps_exec(pod_ip, r"""
Stop-Process -Name ConspitLink2.0 -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 3
Start-Process -FilePath 'C:\Program Files (x86)\Conspit Link 2.0\ConspitLink2.0.exe'
Start-Sleep -Seconds 2
$cl = Get-Process -Name ConspitLink2.0 -ErrorAction SilentlyContinue
if ($cl) { Write-Output "Conspit Link restarted (PID: $($cl.Id))" }
else { Write-Output "WARNING: Conspit Link did not start" }
""")
print(result.get("stdout", "").strip())
print()
print("Done. Conspit Link should now handshake with AC's telemetry.")
print("Check the wheel display on the pod.")
