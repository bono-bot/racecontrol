$cfg = "$env:USERPROFILE\Documents\Assetto Corsa\cfg"
New-Item -ItemType Directory -Path $cfg -Force | Out-Null

# gui.ini — FORCE_START (required for kiosk operation)
$guiContent = @"
[SETTINGS]
FORCE_START=1
HIDE_MAIN_MENU=1
"@
Set-Content -Path "$cfg\gui.ini" -Value $guiContent -Encoding ASCII
Write-Host "gui.ini: FORCE_START=1"

# video.ini — full AC video config for triple 2560x1440@120Hz
# Only write if not present or if it's our minimal bootstrap version (< 200 bytes)
$videoPath = "$cfg\video.ini"
$videoSize = if (Test-Path $videoPath) { (Get-Item $videoPath).Length } else { 0 }
if ($videoSize -lt 200) {
    $videoContent = @"
[ASSETTOCORSA]
HIDE_ARMS=0
HIDE_STEER=0
LOCK_STEER=0
WORLD_DETAIL=5

[CAMERA]
MODE=TRIPLE

[CUBEMAP]
FACES_PER_FRAME=6
FARPLANE=500
SIZE=2048

[EFFECTS]
FXAA=0
MOTION_BLUR=0
RENDER_SMOKE_IN_MIRROR=1
SMOKE=5

[MIRROR]
HQ=1
SIZE=1024

[POST_PROCESS]
DOF=5
ENABLED=1
FILTER=default
FXAA=1
GLARE=5
HEAT_SHIMMER=1
QUALITY=5
RAYS_OF_GOD=1

[REFRESH]
VALUE=120

[SATURATION]
LEVEL=100

[VIDEO]
AAQUALITY=0
AASAMPLES=4
ANISOTROPIC=16
DISABLE_LEGACY_HDR=1
FPS_CAP_MS=0
FULLSCREEN=1
HEIGHT=1440
INDEX=200
REFRESH=120
SHADOW_MAP_SIZE=4096
VSYNC=0
WIDTH=7680
"@
    Set-Content -Path $videoPath -Value $videoContent -Encoding ASCII
    Write-Host "video.ini: REPLACED with full config (triple 7680x1440@120Hz)"
} else {
    Write-Host "video.ini: kept existing ($videoSize bytes)"
}

Write-Host "BOOTSTRAP DONE"
