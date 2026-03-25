@echo off
REM Start rc-sentry-ai - called by schtasks or watchdog
cd /d C:\RacingPoint

REM Log rotation: keep last log, cap at 10MB
if exist rc-sentry-ai.log (
  for %%A in (rc-sentry-ai.log) do if %%~zA GEQ 10485760 (
    echo [%date% %time%] Rotating rc-sentry-ai.log
    del /q rc-sentry-ai-old.log 2>nul
    rename rc-sentry-ai.log rc-sentry-ai-old.log
  )
)

REM Warm up go2rtc H.264 transcoded streams before starting rc-sentry-ai.
REM Without warmup, 13 simultaneous ffmpeg startups overwhelm the NVR.
echo [%date% %time%] Warming go2rtc H.264 streams...
for %%s in (entrance_h264 ch2_h264 reception_wide_h264 reception_h264 ch5_h264 ch6_h264 ch7_h264 ch8_h264 ch9_h264 ch10_h264 ch11_h264 ch12_h264 ch13_h264) do (
  curl.exe -s -m 12 -o nul "http://127.0.0.1:1984/api/frame.jpeg?src=%%s" 1>nul 2>nul
  ping -n 2 127.0.0.1 1>nul
)
echo [%date% %time%] All streams warmed, starting rc-sentry-ai...

rc-sentry-ai.exe
