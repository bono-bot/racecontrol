"""Test AC UDP handshake on a pod via pod-agent."""
import json, urllib.request, base64, sys

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

ps_script = """
$udp = New-Object System.Net.Sockets.UdpClient
$udp.Client.ReceiveTimeout = 2000

# Build AC handshake packet: op=0, id=1, ver=1 (3x i32 LE = 12 bytes)
$pkt = [System.BitConverter]::GetBytes([int32]0) +
        [System.BitConverter]::GetBytes([int32]1) +
        [System.BitConverter]::GetBytes([int32]1)

$ep = New-Object System.Net.IPEndPoint([System.Net.IPAddress]::Parse("127.0.0.1"), 9996)
$udp.Send($pkt, 12, $ep) | Out-Null
Write-Output "Sent handshake to 127.0.0.1:9996"

try {
    $remoteEP = New-Object System.Net.IPEndPoint([System.Net.IPAddress]::Any, 0)
    $response = $udp.Receive([ref]$remoteEP)
    Write-Output "Got response: $($response.Length) bytes"

    # Parse car name from first 200 bytes (UTF-32LE)
    $carBytes = $response[0..199]
    $car = [System.Text.Encoding]::UTF32.GetString($carBytes).TrimEnd([char]0)
    Write-Output "Car: $car"

    # Parse driver name from bytes 200-399
    $driverBytes = $response[200..399]
    $driver = [System.Text.Encoding]::UTF32.GetString($driverBytes).TrimEnd([char]0)
    Write-Output "Driver: $driver"
} catch {
    Write-Output "TIMEOUT - no response from AC: $($_.Exception.Message)"
} finally {
    $udp.Close()
}
"""

print(f"=== AC UDP Handshake Test on {pod_ip} ===")
result = ps_exec(pod_ip, ps_script)
print(f"exit_code: {result['exit_code']}")
print(result.get('stdout', ''))
if result.get('stderr'):
    stderr = result['stderr']
    # Filter out the "Preparing modules" noise
    lines = stderr.split('\n')
    real_errors = [l for l in lines if 'Preparing modules' not in l and l.strip()]
    if real_errors:
        print("STDERR:", '\n'.join(real_errors[:10]))
