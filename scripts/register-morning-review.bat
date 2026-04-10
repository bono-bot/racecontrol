@echo off
schtasks /Create /XML "C:\Users\bono\racingpoint\racecontrol\docs\runbooks\morning-review-task.xml" /TN "MorningReview-Daily"
if %ERRORLEVEL% EQU 0 (
    echo SUCCESS: MorningReview-Daily task registered
) else (
    echo FAILED: exit code %ERRORLEVEL%
)
