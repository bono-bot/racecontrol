"""
Fix AC autospawn on a pod by setting CSP gui.ini FORCE_START=1
and updating race.ini AUTOSPAWN=1, then relaunching.
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

def read_file(pod_ip, path):
    req = urllib.request.Request(f"http://{pod_ip}:8090/file?path={urllib.request.pathname2url(path)}")
    resp = urllib.request.urlopen(req, timeout=10)
    return resp.read().decode('utf-8', errors='replace')

# Step 1: Kill AC
print("[1] Killing AC...")
ps_exec(pod_ip, "Stop-Process -Name acs -Force -ErrorAction SilentlyContinue; Start-Sleep -Seconds 2")

# Step 2: Modify CSP gui.ini - set FORCE_START=1 and HIDE_MAIN_MENU=1
print("[2] Patching CSP gui.ini (FORCE_START=1, HIDE_MAIN_MENU=1)...")
ps_patch_gui = r"""
$guiPath = 'C:\Program Files (x86)\Steam\steamapps\common\assettocorsa\extension\config\gui.ini'
$content = Get-Content $guiPath -Raw
$content = $content -replace 'FORCE_START=0', 'FORCE_START=1'
$content = $content -replace 'HIDE_MAIN_MENU=0', 'HIDE_MAIN_MENU=1'
Set-Content -Path $guiPath -Value $content -Encoding UTF8
Write-Output "Patched gui.ini"
"""
result = ps_exec(pod_ip, ps_patch_gui)
print(f"  {result.get('stdout', '').strip()} (exit={result['exit_code']})")

# Step 3: Write clean race.ini with AUTOSPAWN=1
print("[3] Writing race.ini with AUTOSPAWN=1...")
race_ini = """[AUTOSPAWN]
ACTIVE=1

[BENCHMARK]
ACTIVE=0

[CAR_0]
SETUP=
SKIN=00_default
MODEL=-
MODEL_CONFIG=
BALLAST=0
RESTRICTOR=0
DRIVER_NAME=Test
NATIONALITY=IND
NATION_CODE=IND

[DYNAMIC_TRACK]
LAP_GAIN=0
RANDOMNESS=0
SESSION_START=100
SESSION_TRANSFER=100

[GHOST_CAR]
ENABLED=0
FILE=
LOAD=0
PLAYING=0
RECORDING=0
SECONDS_ADVANTAGE=0

[GROOVE]
VIRTUAL_LAPS=10
MAX_LAPS=30
STARTING_LAPS=0

[HEADER]
VERSION=2

[LAP_INVALIDATOR]
ALLOWED_TYRES_OUT=-1

[LIGHTING]
CLOUD_SPEED=0.200
SUN_ANGLE=16
TIME_MULT=1.0

[OPTIONS]
USE_MPH=0

[RACE]
AI_LEVEL=100
CARS=1
CONFIG_TRACK=
DRIFT_MODE=0
FIXED_SETUP=0
JUMP_START_PENALTY=0
MODEL=ks_ferrari_sf15t
MODEL_CONFIG=
PENALTIES=1
RACE_LAPS=0
SKIN=00_default
TRACK=spa

[REMOTE]
ACTIVE=0
GUID=
NAME=Test
PASSWORD=
SERVER_IP=
SERVER_PORT=
TEAM=

[REPLAY]
ACTIVE=0
FILENAME=

[RESTART]
ACTIVE=0

[SESSION_0]
NAME=Practice
DURATION_MINUTES=60
SPAWN_SET=PIT
TYPE=1
LAPS=0
STARTING_POSITION=1

[TEMPERATURE]
AMBIENT=22
ROAD=28

[WEATHER]
NAME=3_clear

[WIND]
DIRECTION_DEG=0
SPEED_KMH_MAX=0
SPEED_KMH_MIN=0"""

ps_write = f"""$content = @'
{race_ini}
'@
Set-Content -Path 'C:\\Users\\User\\Documents\\Assetto Corsa\\cfg\\race.ini' -Value $content -Encoding ASCII
Write-Output "race.ini written"
"""
result = ps_exec(pod_ip, ps_write)
print(f"  {result.get('stdout', '').strip()} (exit={result['exit_code']})")

# Step 4: Launch AC
print("[4] Launching acs.exe...")
ps_launch = r"Start-Process -FilePath 'C:\Program Files (x86)\Steam\steamapps\common\assettocorsa\acs.exe' -WorkingDirectory 'C:\Program Files (x86)\Steam\steamapps\common\assettocorsa'"
result = ps_exec(pod_ip, ps_launch)
print(f"  Launched (exit={result['exit_code']})")

# Step 5: Verify
print("\nWaiting 5s...")
time.sleep(5)
result = ps_exec(pod_ip, "Get-Process -Name acs -ErrorAction SilentlyContinue | Select-Object Id | Format-List")
if "Id" in result.get("stdout", ""):
    print("AC is RUNNING!")
else:
    print("WARNING: AC not found!")

print("\nDone. CSP FORCE_START=1 should bypass any pre-race screen.")
