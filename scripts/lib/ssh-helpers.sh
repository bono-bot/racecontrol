#!/usr/bin/env bash
# ssh-helpers.sh — Safe remote file operations
#
# Standing rule: "Never pipe SSH output into config files."
# SSH banners (post-quantum warnings, MOTD) go to stderr but some wrappers
# merge streams, silently prepending garbage to the file.
#
# Usage:
#   source scripts/lib/ssh-helpers.sh
#   safe_remote_read user@host /path/to/file.toml local_copy.toml

# safe_remote_read — Copy a remote file via SCP and validate it's not corrupted
#
# Args:
#   $1 — SSH target (user@host)
#   $2 — Remote file path
#   $3 — Local destination path
#
# Returns 0 on success, 1 on failure (with error message to stderr)
safe_remote_read() {
    local target="$1"
    local remote_path="$2"
    local local_path="$3"

    if [ -z "$target" ] || [ -z "$remote_path" ] || [ -z "$local_path" ]; then
        echo "Usage: safe_remote_read user@host /remote/path local_path" >&2
        return 1
    fi

    # Use SCP, never SSH pipe
    if ! scp -q "$target:$remote_path" "$local_path" 2>/dev/null; then
        echo "ERROR: scp failed for $target:$remote_path" >&2
        return 1
    fi

    # Validate first line isn't an SSH banner
    local first_line
    first_line=$(head -1 "$local_path" 2>/dev/null || true)

    # Detect common SSH banner patterns
    if echo "$first_line" | grep -qiE '^(warning|@@@|ssh-|the authenticity|kex_exchange|sntrup)'; then
        echo "ERROR: SSH banner detected in $local_path — file is corrupted!" >&2
        echo "  First line: $first_line" >&2
        rm -f "$local_path"
        return 1
    fi

    # Validate expected format based on file extension
    local ext="${remote_path##*.}"
    case "$ext" in
        toml)
            if ! echo "$first_line" | grep -qE '^\[|^#|^[a-zA-Z_]'; then
                echo "WARNING: $local_path doesn't look like valid TOML (first line: $first_line)" >&2
            fi
            ;;
        json)
            if ! echo "$first_line" | grep -qE '^\{|^\['; then
                echo "WARNING: $local_path doesn't look like valid JSON (first line: $first_line)" >&2
            fi
            ;;
    esac

    return 0
}
