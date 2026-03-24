@echo off
cd /d C:\RacingPoint
taskkill /F /IM rc-sentry.exe 1>nul 2>nul
ping -n 3 127.0.0.1 1>nul
if not exist rc-sentry-new.exe goto :skip_swap
del /Q rc-sentry.exe 1>nul 2>nul
ping -n 2 127.0.0.1 1>nul
if exist rc-sentry.exe del /Q rc-sentry.exe 1>nul 2>nul
move rc-sentry-new.exe rc-sentry.exe 1>nul
:skip_swap
start /D C:\RacingPoint "" C:\RacingPoint\rc-sentry.exe
