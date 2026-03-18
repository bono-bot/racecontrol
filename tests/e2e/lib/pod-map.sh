#!/bin/bash
# lib/pod-map.sh
# Single source of truth for pod IP addresses.
# Source this file in any script that needs to reach a specific pod.
# Usage: POD_IP=$(pod_ip pod-8)

pod_ip() {
  case "$1" in
    pod-1) echo "192.168.31.89" ;;
    pod-2) echo "192.168.31.33" ;;
    pod-3) echo "192.168.31.28" ;;
    pod-4) echo "192.168.31.88" ;;
    pod-5) echo "192.168.31.86" ;;
    pod-6) echo "192.168.31.87" ;;
    pod-7) echo "192.168.31.38" ;;
    pod-8) echo "192.168.31.91" ;;
    *)
      echo "" >&2
      echo "ERROR: Unknown pod '$1'. Valid: pod-1 through pod-8." >&2
      return 1
      ;;
  esac
}
