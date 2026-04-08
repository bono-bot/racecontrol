#!/bin/bash
# Add missing game sections to all pod TOMLs
# Based on actual Steam installations discovered 2026-04-09

STEAM_COMMON='C:\\Program Files (x86)\\Steam\\steamapps\\common'

# Game definitions: id|steam_app_id|exe_relative_path
GAMES=(
  "assetto_corsa_evo|3058630|Assetto Corsa EVO\\AssettoCorsaEVO.exe"
  "assetto_corsa_rally|3917090|Assetto Corsa Rally\\acr\\Binaries\\Win64\\acr.exe"
  "le_mans_ultimate|2399420|Le Mans Ultimate\\Le Mans Ultimate.exe"
  "forza_horizon_5|1551360|ForzaHorizon5\\ForzaHorizon5.exe"
)

# Pod installations (0=not installed, 1=installed)
# Pods:                    1 2 3 4 5 6 7 8
ACE_PODS=(                 1 1 1 1 1 0 1 1)
ACR_PODS=(                 1 0 1 0 1 0 1 1)
LMU_PODS=(                 0 1 0 1 0 1 1 0)
FH5_PODS=(                 1 0 0 0 0 0 0 0)

DEPLOY_DIR="deploy/configs"

for pod in 1 2 3 4 5 6 7 8; do
  TOML_FILE="$DEPLOY_DIR/rc-agent-pod${pod}.toml"

  if [ ! -f "$TOML_FILE" ]; then
    echo "SKIP: $TOML_FILE not found"
    continue
  fi

  echo "=== Pod $pod ==="
  idx=$((pod - 1))

  # AC Evo
  if [ "${ACE_PODS[$idx]}" = "1" ] && ! grep -q "games.assetto_corsa_evo" "$TOML_FILE"; then
    echo "  Adding AC Evo"
    cat >> "$TOML_FILE" << 'GAME_EOF'

[games.assetto_corsa_evo]
exe_path = "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Assetto Corsa EVO\\AssettoCorsaEVO.exe"
steam_app_id = 3058630
use_steam = true
GAME_EOF
  fi

  # AC Rally
  if [ "${ACR_PODS[$idx]}" = "1" ] && ! grep -q "games.assetto_corsa_rally" "$TOML_FILE"; then
    echo "  Adding AC Rally"
    cat >> "$TOML_FILE" << 'GAME_EOF'

[games.assetto_corsa_rally]
exe_path = "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Assetto Corsa Rally\\acr\\Binaries\\Win64\\acr.exe"
steam_app_id = 3917090
use_steam = true
GAME_EOF
  fi

  # Le Mans Ultimate
  if [ "${LMU_PODS[$idx]}" = "1" ] && ! grep -q "games.le_mans_ultimate" "$TOML_FILE"; then
    echo "  Adding Le Mans Ultimate"
    cat >> "$TOML_FILE" << 'GAME_EOF'

[games.le_mans_ultimate]
exe_path = "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Le Mans Ultimate\\Le Mans Ultimate.exe"
steam_app_id = 2399420
use_steam = true
GAME_EOF
  fi

  # Forza Horizon 5
  if [ "${FH5_PODS[$idx]}" = "1" ] && ! grep -q "games.forza_horizon_5" "$TOML_FILE"; then
    echo "  Adding Forza Horizon 5"
    cat >> "$TOML_FILE" << 'GAME_EOF'

[games.forza_horizon_5]
exe_path = "C:\\Program Files (x86)\\Steam\\steamapps\\common\\ForzaHorizon5\\ForzaHorizon5.exe"
steam_app_id = 1551360
use_steam = true
GAME_EOF
  fi

done

echo ""
echo "Done. Deploy with: scp deploy/configs/rc-agent-pod*.toml to each pod"
