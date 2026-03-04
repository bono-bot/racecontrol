"""
Launch Assetto Corsa on a pod with auto-spawn + Conspit Link restart.
Usage: python pod_ac_launch.py <pod_ip> [car] [track] [driver_name]

Flow: Kill AC -> Write race.ini (AUTOSPAWN=1) -> Launch acs.exe -> Restart Conspit Link
Requires: CSP gui.ini already patched with FORCE_START=1 (run fix_autospawn.py first)
"""
import json, urllib.request, base64, sys, time

pod_ip = sys.argv[1] if len(sys.argv) > 1 else "192.168.31.91"
car = sys.argv[2] if len(sys.argv) > 2 else "ks_ferrari_sf15t"
track = sys.argv[3] if len(sys.argv) > 3 else "spa"
driver = sys.argv[4] if len(sys.argv) > 4 else "Test"

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

race_ini = f"""[AUTOSPAWN]
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
DRIVER_NAME={driver}
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
MODEL={car}
MODEL_CONFIG=
PENALTIES=1
RACE_LAPS=0
SKIN=00_default
TRACK={track}

[REMOTE]
ACTIVE=0
GUID=
NAME={driver}
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

print(f"=== AC Launch: {car} @ {track} on {pod_ip} ===")

# Step 1: Kill AC
print("[1/5] Killing AC...")
ps_exec(pod_ip, "Stop-Process -Name acs -Force -ErrorAction SilentlyContinue; Stop-Process -Name AssettoCorsa -Force -ErrorAction SilentlyContinue; Start-Sleep -Seconds 2")

# Step 2: Write race.ini
print("[2/5] Writing race.ini...")
ps_write = f"""$content = @'
{race_ini}
'@
Set-Content -Path 'C:\\Users\\User\\Documents\\Assetto Corsa\\cfg\\race.ini' -Value $content -Encoding ASCII
"""
ps_exec(pod_ip, ps_write)

# Step 3: Launch acs.exe
print("[3/5] Launching acs.exe...")
ps_exec(pod_ip, r"Start-Process -FilePath 'C:\Program Files (x86)\Steam\steamapps\common\assettocorsa\acs.exe' -WorkingDirectory 'C:\Program Files (x86)\Steam\steamapps\common\assettocorsa'")

# Step 4: Wait for AC to load, then restart Conspit Link
print("[4/5] Waiting 8s for AC to load...")
time.sleep(8)

print("[5/5] Restarting Conspit Link 2.0...")
result = ps_exec(pod_ip, r"""
Stop-Process -Name ConspitLink2.0 -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 2
Start-Process -FilePath 'C:\Program Files (x86)\Conspit Link 2.0\ConspitLink2.0.exe'
Start-Sleep -Seconds 2
$acs = Get-Process -Name acs -ErrorAction SilentlyContinue
$cl = Get-Process -Name ConspitLink2.0 -ErrorAction SilentlyContinue
Write-Output "AC: $($acs -ne $null) | Conspit: $($cl -ne $null)"
""")
print(f"  {result.get('stdout', '').strip()}")

print()
print("Done! Car should be on track with wheel display active.")
