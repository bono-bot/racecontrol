@echo off
REM start-racecontrol-direct.bat
REM Invoked by the StartRCDirect scheduled task (Run As: SYSTEM, Logon Mode: Interactive/Background).
REM StartRCDirect is the restart path used by racecontrol-watchdog.bat when it detects racecontrol.exe
REM is not running. The `start` command is REQUIRED - running racecontrol.exe directly in this script's
REM inherited cmd.exe context (as a child of Task Scheduler in Session 0) causes racecontrol to exit
REM with code 1 on its first stdout write because there is no real console attached.
REM
REM The `start` command spawns racecontrol in its own process with its own console handle, then cmd.exe
REM exits immediately. racecontrol persists as a detached process.
REM
REM Incident (2026-04-11 20:23-20:50 IST): racecontrol crash-looped for 25+ min because this bat ran
REM racecontrol.exe directly and the schtask was also set to "Logon Mode: Interactive only" + "Run As
REM ADMIN" (silently no-ops with no interactive login). Both issues fixed together. See provision-
REM startrcdirect.ps1 for the schtask side.

cd /d C:\RacingPoint
start "RaceControl" /D C:\RacingPoint C:\RacingPoint\racecontrol.exe
