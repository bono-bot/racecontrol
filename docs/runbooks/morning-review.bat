@echo off
for /f "usebackq tokens=1,2 delims==" %%a in ("%USERPROFILE%\.claude\comms-link.env") do set %%a=%%b
cd /D C:\Users\bono\racingpoint\comms-link
node send-message.js "Morning review -- check incident log: https://docs.google.com/spreadsheets/d/PLACEHOLDER-pending-creation/edit?usp=sharing"
