#!/usr/bin/env bash
# Phase 335: Extract track outlines from AC fast_lane.ai files on a pod.
#
# Usage (run on a pod via SSH or exec):
#   bash scripts/extract-track-outlines.sh [ac_content_root]
#
# Default AC content root: C:/Program Files (x86)/Steam/steamapps/common/assettocorsa/content
#
# This script finds all fast_lane.ai files, reads the point count,
# and creates minimal JSON files that the server can parse at startup.
# For full parsing (normalization, downsampling), the server's Rust module
# does the heavy lifting — this script is just a discovery/listing tool.

set -euo pipefail

AC_ROOT="${1:-/c/Program Files (x86)/Steam/steamapps/common/assettocorsa/content}"

echo "=== Track Outline Discovery ==="
echo "AC content root: $AC_ROOT"
echo ""

TRACK_DIR="$AC_ROOT/tracks"
if [ ! -d "$TRACK_DIR" ]; then
    echo "ERROR: Track directory not found: $TRACK_DIR"
    exit 1
fi

count=0
for track_dir in "$TRACK_DIR"/*/; do
    track_id=$(basename "$track_dir")

    # Default layout: tracks/{track_id}/ai/fast_lane.ai
    ai_file="$track_dir/ai/fast_lane.ai"
    if [ -f "$ai_file" ]; then
        size=$(stat -c%s "$ai_file" 2>/dev/null || wc -c < "$ai_file")
        echo "  $track_id (default): $ai_file [$size bytes]"
        count=$((count + 1))
    fi

    # Named configs: tracks/{track_id}/{config}/ai/fast_lane.ai
    for config_dir in "$track_dir"/*/; do
        config_name=$(basename "$config_dir")
        config_ai="$config_dir/ai/fast_lane.ai"
        if [ -f "$config_ai" ]; then
            size=$(stat -c%s "$config_ai" 2>/dev/null || wc -c < "$config_ai")
            echo "  $track_id/$config_name: $config_ai [$size bytes]"
            count=$((count + 1))
        fi
    done
done

echo ""
echo "Found $count fast_lane.ai files"
echo ""
echo "To extract these into JSON outlines, the server will parse them"
echo "on first request via GET /api/v1/spectator/track/{track_id}."
echo "Or copy the fast_lane.ai files to the server and use the extract_and_cache() API."
