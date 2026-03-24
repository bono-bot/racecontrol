@echo off
title RC-Sentry
cd /d C:\RacingPoint
taskkill /F /IM rc-sentry.exe 1>/dev/null 2>/dev/null
timeout /t 2 /nobreak 1>/dev/null
if not exist rc-sentry-new.exe goto :startsentry
del /Q rc-sentry.exe 1>/dev/null 2>/dev/null
timeout /t 1 /nobreak 1>/dev/null
if exist rc-sentry.exe del /Q rc-sentry.exe 1>/dev/null 2>/dev/null
move rc-sentry-new.exe rc-sentry.exe 1>/dev/null
:startsentry
start /D C:\RacingPoint "" C:\RacingPoint\rc-sentry.exe
