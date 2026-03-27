@echo off
REM ============================================================
REM set-pagefile.bat — Set fixed pagefile to prevent fragmentation
REM during long races. Run ONCE as admin on each pod.
REM Sets pagefile to 24GB (1.5x 16GB RAM) on C: drive.
REM ============================================================

wmic computersystem set AutomaticManagedPagefile=False
wmic pagefileset where "name='C:\\pagefile.sys'" set InitialSize=24576,MaximumSize=24576
echo Pagefile set to 24GB fixed. Reboot required.
pause
