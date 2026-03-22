#!/usr/bin/env bash
set -euo pipefail

# Racing Point VPS — 2GB Swap Setup
# Run once: bash /root/racingpoint/racecontrol/cloud/setup-swap.sh

if swapon --show | grep -q '/swapfile'; then
  echo "Swap already configured:"
  swapon --show
  exit 0
fi

echo "Creating 2GB swap..."
fallocate -l 2G /swapfile
chmod 600 /swapfile
mkswap /swapfile
swapon /swapfile

# Make persistent across reboots
if ! grep -q '/swapfile' /etc/fstab; then
  echo '/swapfile none swap sw 0 0' >> /etc/fstab
fi

echo "Swap configured:"
swapon --show
free -h
