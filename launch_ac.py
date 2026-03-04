import json, urllib.request, base64
import sys

pod_ip = sys.argv[1] if len(sys.argv) > 1 else "192.168.31.91"
action = sys.argv[2] if len(sys.argv) > 2 else "write_and_launch"

race_ini = r"""[AUTOSPAWN]
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
SPEED_KMH_MIN=0
"""

def exec_on_pod(cmd):
    data = json.dumps({"cmd": cmd}).encode()
    req = urllib.request.Request(f"http://{pod_ip}:8090/exec", data=data, headers={"Content-Type": "application/json"})
    resp = urllib.request.urlopen(req, timeout=30)
    return json.loads(resp.read().decode())

def ps_exec(script):
    encoded = base64.b64encode(script.encode('utf-16-le')).decode('ascii')
    return exec_on_pod(f"cd /d C:\\RacingPoint & powershell -EncodedCommand {encoded}")

if action in ("write", "write_and_launch"):
    # Write race.ini
    ps_write = f"""$content = @'
{race_ini.strip()}
'@
Set-Content -Path 'C:\\Users\\User\\Documents\\Assetto Corsa\\cfg\\race.ini' -Value $content -Encoding ASCII
"""
    result = ps_exec(ps_write)
    print(f"Write race.ini: exit={result['exit_code']}")

if action in ("launch", "write_and_launch"):
    # Kill existing AC first
    ps_kill = "Stop-Process -Name acs -Force -ErrorAction SilentlyContinue; Start-Sleep -Seconds 1"
    ps_exec(ps_kill)
    print("Killed existing AC")

    # Launch acs.exe
    ps_launch = r"Start-Process -FilePath 'C:\Program Files (x86)\Steam\steamapps\common\assettocorsa\acs.exe' -WorkingDirectory 'C:\Program Files (x86)\Steam\steamapps\common\assettocorsa'"
    result = ps_exec(ps_launch)
    print(f"Launch AC: exit={result['exit_code']}")
