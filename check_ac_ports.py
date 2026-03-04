"""Check AC UDP port bindings on a pod."""
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

print(f"=== AC Port Check on {pod_ip} ===\n")

# Check port 9996
result = ps_exec('netstat -anop UDP | Select-String 9996')
print("Port 9996 UDP:")
print(result.get('stdout', '') or '  No match')

# Check all UDP ports for acs.exe
result2 = ps_exec('netstat -anop UDP | Select-String 2604')
print("\nAC (PID 2604) UDP ports:")
print(result2.get('stdout', '') or '  No match')

# Check all UDP listeners
result3 = ps_exec('netstat -anop UDP | Select-String "LISTEN|0.0.0.0|127.0.0"')
print("\nAll UDP bindings:")
stdout = result3.get('stdout', '')
if stdout:
    for line in stdout.strip().split('\n')[:20]:
        print(f"  {line.strip()}")
else:
    print("  No match")

# Check Conspit Link port usage
result4 = ps_exec('$cl = Get-Process -Name ConspitLink2.0 -ErrorAction SilentlyContinue; if ($cl) { netstat -anop UDP | Select-String $cl.Id } else { Write-Output "ConspitLink not running" }')
print("\nConspit Link UDP ports:")
print(result4.get('stdout', '') or '  No match')
