@echo off
REM Start go2rtc camera proxy on James (.27)
REM Deploy to C:\RacingPoint\go2rtc\start-go2rtc.bat
REM Registered as HKLM Run key for boot start

cd /d C:\RacingPoint\go2rtc

REM Kill existing go2rtc instances
taskkill /F /IM go2rtc.exe >nul 2>&1
ping -n 3 127.0.0.1 >nul

REM Verify ffmpeg exists (required for NVR H.264 transcoding)
if not exist "C:\RacingPoint\go2rtc\ffmpeg.exe" (
    echo [%date% %time%] WARNING: ffmpeg.exe missing - NVR streams will fail
)

REM Start go2rtc with config
start "go2rtc" /MIN go2rtc.exe -config go2rtc.yaml

REM Wait and verify
ping -n 5 127.0.0.1 >nul
curl.exe -s -m 5 http://127.0.0.1:1984/api/streams >nul 2>&1
if %ERRORLEVEL% equ 0 (
    echo [%date% %time%] go2rtc started on :1984
) else (
    echo [%date% %time%] WARNING: go2rtc may not have started
)
