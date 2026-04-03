#!/usr/bin/env bash
# build-sentry-ai.sh — Build rc-sentry-ai with dynamic CRT
#
# rc-sentry-ai MUST use dynamic CRT due to ONNX Runtime (ort crate).
# The workspace .cargo/config.toml sets +crt-static globally for all
# other crates, but ONNX's DirectML/CUDA objects expect ucrt symbols
# from the dynamic CRT. Building with +crt-static causes unresolved
# external linker errors (__imp_tolower, etc.).
#
# Usage:
#   bash scripts/build-sentry-ai.sh
#   bash scripts/build-sentry-ai.sh --release

set -euo pipefail

MODE="${1:---release}"
SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
REPO_ROOT=$(cd "$SCRIPT_DIR/.." && pwd)

echo "Building rc-sentry-ai with DYNAMIC CRT (overriding workspace +crt-static)"
echo "  Mode: $MODE"

cd "$REPO_ROOT"
RUSTFLAGS="-C target-feature=-crt-static" cargo build $MODE --bin rc-sentry-ai

echo ""
echo "Build complete. Binary requires Visual C++ Redistributable on target machine."
echo "Deploy to James (.27) only — rc-sentry-ai runs on James's RTX 4070, not on pods."
