# Deploy Files

These files should be copied to `C:\RacingPoint\` on each pod alongside rc-agent.exe:

## Required
- `steam_appid.txt` — Contains "480" (Steamworks Spacewar test app ID). Prevents Steam
  from terminating rc-agent as an "unauthorized" process accessing game shared memory.
  VMS Connect ships this same file for the same reason.

- `launch-ac.bat` — Isolated AC launcher subprocess. rc-agent spawns this as a separate
  process to launch acs.exe. If CSP/Steam/anti-cheat kills the launcher, rc-agent survives.
  VMS uses the same pattern (SimLauncher.exe is a separate disposable process).

- `rc-agent.exe.manifest` — Windows application manifest declaring `requestedExecutionLevel=asInvoker`.
  Prevents anti-cheat from flagging rc-agent as elevated. VMS Connect uses the same pattern.
  Must be placed next to rc-agent.exe (Windows reads `<exe>.manifest` automatically).

## Optional
- `nircmdc.exe` — NirSoft NirCmd command-line tool for window management (focus, topmost,
  activation). VMS ships this to avoid Win32 API calls from the agent process. Download
  from https://www.nirsoft.net/utils/nircmd.html if needed.

## AC Plugin (per pod)
- Copy `plugins/assetto_corsa/` contents to `<AC>/apps/python/RaceControl/`
- Enable the RaceControl plugin in AC settings or via gui.ini
