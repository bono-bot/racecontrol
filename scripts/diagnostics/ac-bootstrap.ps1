$cfg = "$env:USERPROFILE\Documents\Assetto Corsa\cfg"
New-Item -ItemType Directory -Path $cfg -Force | Out-Null
Set-Content -Path "$cfg\gui.ini" -Value "[SETTINGS]`nFORCE_START=1`nHIDE_MAIN_MENU=1" -Encoding ASCII
Set-Content -Path "$cfg\video.ini" -Value "[VIDEO]`nFULLSCREEN=1`nWIDTH=1920`nHEIGHT=1080`nREFRESH=60`nVSYNC=1`nAASAMPLES=2`nANISOTROPIC=8`nSHADOW_MAP_SIZE=2048`nWORLD_DETAIL=1`nSMOKE=1" -Encoding ASCII
Set-Content -Path "$cfg\controls.ini" -Value "[FF]`nGAIN=70`nMIN_FORCE=0.05" -Encoding ASCII
Get-ChildItem $cfg | Select-Object Name
