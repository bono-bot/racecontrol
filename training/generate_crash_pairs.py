#!/usr/bin/env python3
"""Generate crash scenario training pairs — cross-product of sims x error types x pods."""

import json
import random
from pathlib import Path

OUTPUT_PATH = Path(__file__).parent / "data" / "crash_pairs.json"

PODS = [
    {"num": 1, "ip": "192.168.31.89"},
    {"num": 2, "ip": "192.168.31.33"},
    {"num": 3, "ip": "192.168.31.28"},
    {"num": 4, "ip": "192.168.31.88"},
    {"num": 5, "ip": "192.168.31.86"},
    {"num": 6, "ip": "192.168.31.87"},
    {"num": 7, "ip": "192.168.31.38"},
    {"num": 8, "ip": "192.168.31.91"},
]

SIMS = [
    {"name": "Assetto Corsa", "short": "AC", "exe": "acs.exe", "udp": 9996},
    {"name": "F1 25", "short": "F1", "exe": "F1_25.exe", "udp": 20777},
    {"name": "iRacing", "short": "iR", "exe": "iRacingSim64DX11.exe", "udp": 6789},
    {"name": "Le Mans Ultimate", "short": "LMU", "exe": "LMU.exe", "udp": 5555},
    {"name": "Forza Motorsport", "short": "Forza", "exe": "ForzaMotorsport.exe", "udp": 5300},
]

ERROR_TYPES = [
    {
        "type": "gpu_crash",
        "contexts": [
            "exit code -1073741819 (access violation)",
            "exit code -1073740791 (STATUS_STACK_BUFFER_OVERRUN)",
            "GPU device lost, DXGI_ERROR_DEVICE_REMOVED",
            "display driver nvlddmkm stopped responding and has successfully recovered",
        ],
        "diagnosis": (
            "GPU crash indicators. This is likely a driver crash or VRAM exhaustion.\n\n"
            "Immediate steps:\n"
            "1. Check if other games were running: `tasklist | findstr -i \"acs F1_ iRacing LMU Forza\"`\n"
            "2. Check GPU temperature: `nvidia-smi` (if >85°C, check pod cooling)\n"
            "3. Check VRAM usage: `nvidia-smi` (if close to max, reduce graphics settings)\n\n"
            "If recurring on same pod:\n"
            "- Update GPU drivers via GeForce Experience\n"
            "- Run `sfc /scannow` to check for system file corruption\n"
            "- Check for thermal throttling — clean dust from GPU heatsink\n"
            "- Underclock GPU slightly via MSI Afterburner as temporary fix"
        ),
    },
    {
        "type": "oom",
        "contexts": [
            "exit code -1073741801 (insufficient memory)",
            "out of memory: failed to allocate",
            "VirtualAlloc failed with error 8: Not enough memory",
            "Windows ran out of virtual memory",
        ],
        "diagnosis": (
            "Out of memory crash. The game exceeded available RAM or virtual memory.\n\n"
            "Immediate steps:\n"
            "1. Check current memory: `wmic OS get FreePhysicalMemory /Format:Value`\n"
            "2. Kill any lingering processes: `tasklist /FI \"MEMUSAGE gt 500000\"`\n"
            "3. Check for zombie game processes from previous session\n\n"
            "If recurring:\n"
            "- Check pagefile size: Settings → System → About → Advanced system settings → Performance → Virtual Memory\n"
            "- Reduce game graphics settings (especially texture quality)\n"
            "- Check for memory leaks in ConspitLink2.0 (restart it)\n"
            "- Restart the pod to clear accumulated memory fragmentation"
        ),
    },
    {
        "type": "mod_conflict",
        "contexts": [
            "mod loading error: incompatible CSP version",
            "content manager error: missing track extension",
            "failed to load car skin: file not found",
            "shader compilation failed: custom shader error",
        ],
        "diagnosis": (
            "Content/mod conflict. A modded asset or CSP extension caused the crash.\n\n"
            "Immediate steps:\n"
            "1. Check which track/car was selected in the last session\n"
            "2. Try launching with a known-good track/car combo (e.g., Monza + Ferrari 488)\n"
            "3. If AC: check Content Manager logs at `%LOCALAPPDATA%\\AcTools Content Manager\\Logs`\n\n"
            "If recurring with specific content:\n"
            "- Verify mod integrity: right-click in CM → Verify files\n"
            "- Update Custom Shaders Patch (CSP) to latest version\n"
            "- Remove or reinstall the problematic mod\n"
            "- Check game_launch_events table for which content was being loaded"
        ),
    },
    {
        "type": "corrupt_files",
        "contexts": [
            "failed to read game data: CRC mismatch",
            "steam_api64.dll is missing or corrupt",
            "EasyAntiCheat integrity check failed",
            "fatal error: configuration file corrupt or missing",
        ],
        "diagnosis": (
            "Corrupted game files detected.\n\n"
            "Immediate steps:\n"
            "1. Verify game files through the platform (Steam: right-click → Properties → Verify integrity)\n"
            "2. For AC: also verify CSP and Content Manager installation\n"
            "3. Check disk health: `wmic diskdrive get status` (should show 'OK')\n\n"
            "If disk issues found:\n"
            "- Run `chkdsk C: /f` (requires reboot)\n"
            "- Check SMART status: `wmic diskdrive get model,status`\n"
            "- Consider replacing the drive if recurring\n\n"
            "Quick workaround: reinstall the game if verify doesn't fix it"
        ),
    },
    {
        "type": "network",
        "contexts": [
            "connection to multiplayer server timed out",
            "Steam overlay: no connection",
            "failed to connect to iRacing service",
            "network error: DNS resolution failed",
        ],
        "diagnosis": (
            "Network connectivity issue affecting the game.\n\n"
            "Immediate steps:\n"
            "1. Check pod internet: `ping 8.8.8.8` from the pod\n"
            "2. Check local network: `ping 192.168.31.1` (router) and `ping 192.168.31.51` (server)\n"
            "3. Check DNS: `nslookup google.com`\n"
            "4. Check if Steam/game service is down: check status pages\n\n"
            "If local network OK but internet down:\n"
            "- Check router (192.168.31.1) WAN connection\n"
            "- Check if ISP is down\n"
            "- For multiplayer: AC server runs locally on .51, doesn't need internet\n\n"
            "For Steam issues: `taskkill /F /IM steam.exe` then relaunch"
        ),
    },
    {
        "type": "wheelbase_disconnect",
        "contexts": [
            "USB device disconnected during gameplay",
            "OpenFFBoard HID device not found",
            "force feedback error: device not responding",
            "controller disconnected: no input devices",
        ],
        "diagnosis": (
            "Wheelbase disconnected during gameplay. Conspit Ares 8Nm (OpenFFBoard VID:0x1209 PID:0xFFB0).\n\n"
            "Immediate steps:\n"
            "1. Check physical USB connection — try different port\n"
            "2. Restart ConspitLink2.0: `taskkill /F /IM ConspitLink2.0.exe`\n"
            "   Then relaunch: `start \"\" \"C:\\Program Files (x86)\\Conspit Link 2.0\\ConspitLink2.0.exe\"`\n"
            "3. Check Device Manager for the OpenFFBoard device\n"
            "4. Power-cycle the wheelbase (power strip under the rig)\n\n"
            "If USB hub issues:\n"
            "- Check if multiple USB devices are drawing too much power\n"
            "- Try connecting wheelbase directly to motherboard USB (not hub)\n"
            "- Check `dmesg`-equivalent in Event Viewer → System for USB errors\n\n"
            "rc-agent driving detector will report NoDevice state when wheelbase disconnects."
        ),
    },
]


def generate_crash_pairs() -> list[dict]:
    """Generate crash scenario training pairs."""
    pairs = []

    for sim in SIMS:
        for error in ERROR_TYPES:
            # Pick 2 random contexts and 2 random pods for each sim x error combo
            contexts = random.sample(error["contexts"], min(2, len(error["contexts"])))
            pods = random.sample(PODS, 2)

            for ctx, pod in zip(contexts, pods):
                # Varied instruction formats
                instructions = [
                    f"{sim['name']} crashed on pod {pod['num']}. Error: {ctx}",
                    f"Pod {pod['num']} ({pod['ip']}): {sim['exe']} exited unexpectedly. Context: {ctx}",
                ]
                instruction = random.choice(instructions)

                output = (
                    f"**{sim['name']} crash on Pod {pod['num']}** ({pod['ip']})\n\n"
                    f"Error context: {ctx}\n\n"
                    f"{error['diagnosis']}\n\n"
                    f"After resolving, relaunch {sim['exe']} and verify telemetry on UDP {sim['udp']}."
                )

                pairs.append({
                    "instruction": instruction,
                    "input": "",
                    "output": output,
                    "source": f"crash/{error['type']}/{sim['short']}",
                })

    return pairs


if __name__ == "__main__":
    pairs = generate_crash_pairs()

    OUTPUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    with open(OUTPUT_PATH, "w", encoding="utf-8") as f:
        json.dump(pairs, f, indent=2, ensure_ascii=False)

    print(f"Generated {len(pairs)} crash scenario pairs -> {OUTPUT_PATH}")
