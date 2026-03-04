"""Check CSP telemetry/UDP config on a pod."""
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

def read_file(path):
    req = urllib.request.Request(f"http://{pod_ip}:8090/file?path={path}")
    try:
        resp = urllib.request.urlopen(req, timeout=10)
        return resp.read().decode('utf-8', errors='replace')
    except Exception as e:
        return f"ERROR: {e}"

print(f"=== CSP Config Check on {pod_ip} ===\n")

# List CSP config files
result = ps_exec(r'Get-ChildItem "C:\Program Files (x86)\Steam\steamapps\common\assettocorsa\extension\config" -Name')
print("CSP config files:")
print(result.get('stdout', ''))

# Check for UDP/telemetry related configs
for fname in ['general.ini', 'data_output.ini', 'gui.ini', 'network.ini']:
    path = f"C:/Program Files (x86)/Steam/steamapps/common/assettocorsa/extension/config/{fname}"
    content = read_file(path)
    if 'ERROR' not in content:
        # Show only UDP/telemetry related sections
        lines = content.split('\n')
        relevant = []
        in_section = False
        for line in lines:
            if any(kw in line.upper() for kw in ['UDP', 'TELEMETRY', 'OUTPUT', 'BROADCAST', 'PORT', 'CODEMASTERS', 'F1']):
                relevant.append(line)
                in_section = True
            elif line.startswith('['):
                if in_section:
                    relevant.append('')
                in_section = any(kw in line.upper() for kw in ['UDP', 'TELEMETRY', 'OUTPUT', 'BROADCAST', 'PORT', 'CODEMASTERS', 'F1', 'DATA'])
                if in_section:
                    relevant.append(line)
            elif in_section:
                relevant.append(line)
        if relevant:
            print(f"\n--- {fname} (relevant sections) ---")
            print('\n'.join(relevant[:50]))
        else:
            print(f"\n--- {fname}: no UDP/telemetry config found ---")
    else:
        print(f"\n--- {fname}: {content} ---")
