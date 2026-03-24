@echo off
REM Open the Racing Point Kiosk in Edge kiosk mode (fullscreen, no chrome)
REM Deploy to C:\RacingPoint\open-kiosk.bat on Server .23
REM Also create a desktop shortcut pointing to this bat

set KIOSK_URL=http://localhost:8080/kiosk

REM --kiosk launches Edge in fullscreen kiosk mode (no address bar, no tabs)
REM --edge-kiosk-type=fullscreen for full immersive mode
start "" "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe" --kiosk "%KIOSK_URL%" --edge-kiosk-type=fullscreen --no-first-run --disable-features=msEdgeSidebarV2
