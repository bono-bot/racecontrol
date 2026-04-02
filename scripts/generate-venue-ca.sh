#!/usr/bin/env bash
# generate-venue-ca.sh — RacingPoint Venue CA + cert generation
#
# Generates a self-signed venue Certificate Authority and issues:
#   - server cert  (for racecontrol :8080 mTLS)
#   - pod-1..8 client certs  (for rc-agent mTLS client auth)
#
# Usage:
#   bash scripts/generate-venue-ca.sh                     # default output: C:\RacingPoint\tls (or /etc/racingpoint/tls on Linux)
#   bash scripts/generate-venue-ca.sh --output /tmp/tls   # custom output dir
#   bash scripts/generate-venue-ca.sh --force             # regenerate all (overwrite existing)
#
# Requirements: openssl (any modern version, 1.1+)
#
# Output files:
#   <out>/ca.pem             — venue CA certificate (distribute to all clients/pods)
#   <out>/ca-key.pem         — venue CA private key (keep secret, server only)
#   <out>/server.pem         — server certificate (signed by venue CA)
#   <out>/server-key.pem     — server private key
#   <out>/pod-N.pem          — per-pod client certificate (signed by venue CA), N=1..8
#   <out>/pod-N-key.pem      — per-pod private key
#   <out>/bundle.pem         — server cert + CA cert chain (for TLS handshake)

set -euo pipefail

# ── defaults ──────────────────────────────────────────────────────────────────

# Default output dir: Windows path when running under Git Bash/WSL, Linux path otherwise
if [[ "$(uname -s)" =~ MINGW|CYGWIN|MSYS ]]; then
    DEFAULT_OUT="C:/RacingPoint/tls"
else
    DEFAULT_OUT="/etc/racingpoint/tls"
fi

OUTPUT_DIR="$DEFAULT_OUT"
FORCE=0
POD_COUNT=8

# Server SANs — add all known server IPs/hostnames here
SERVER_SANS="IP:192.168.31.23,IP:127.0.0.1,DNS:localhost,DNS:racing-point-server,DNS:racing-point-server-1"
# Also include common development / Bono VPS entries
SERVER_SANS_EXTENDED="${SERVER_SANS},IP:100.125.108.37,IP:100.70.177.44"

CA_DAYS=3650       # 10 years
CERT_DAYS=1825     # 5 years
KEY_BITS=2048      # 2048-bit RSA (fast, adequate for internal LAN)

# ── arg parsing ───────────────────────────────────────────────────────────────

while [[ $# -gt 0 ]]; do
    case "$1" in
        --output|-o)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        --force|-f)
            FORCE=1
            shift
            ;;
        --pods)
            POD_COUNT="$2"
            shift 2
            ;;
        --help|-h)
            grep '^#' "$0" | head -30 | sed 's/^# \?//'
            exit 0
            ;;
        *)
            echo "Unknown argument: $1" >&2
            exit 1
            ;;
    esac
done

# ── helpers ───────────────────────────────────────────────────────────────────

need() { command -v "$1" &>/dev/null || { echo "ERROR: '$1' not found in PATH" >&2; exit 1; }; }
info()  { echo "[INFO]  $*"; }
ok()    { echo "[OK]    $*"; }
skip()  { echo "[SKIP]  $*"; }
warn()  { echo "[WARN]  $*"; }

need openssl

mkdir -p "$OUTPUT_DIR"
cd "$OUTPUT_DIR"

# ── CA generation ─────────────────────────────────────────────────────────────

if [[ -f ca.pem && -f ca-key.pem && "$FORCE" -eq 0 ]]; then
    skip "CA already exists — use --force to regenerate"
else
    info "Generating venue CA (${KEY_BITS}-bit RSA, ${CA_DAYS} days)..."
    openssl genrsa -out ca-key.pem "$KEY_BITS" 2>/dev/null
    openssl req -new -x509 \
        -key ca-key.pem \
        -out ca.pem \
        -days "$CA_DAYS" \
        -subj "/O=RacingPoint/CN=RacingPoint Venue CA" \
        -extensions v3_ca \
        -addext "basicConstraints=critical,CA:TRUE,pathlen:0" \
        -addext "keyUsage=critical,keyCertSign,cRLSign" \
        -addext "subjectKeyIdentifier=hash"
    ok "CA certificate generated: ca.pem"
fi

# Helper: issue a certificate signed by the venue CA
# issue_cert <name> <cn> <san_string> <is_client>
# san_string example: "IP:192.168.31.23,DNS:localhost"
# is_client: "yes" adds clientAuth EKU, "no" adds serverAuth only
issue_cert() {
    local name="$1"
    local cn="$2"
    local san="$3"
    local is_client="${4:-no}"

    local cert_file="${name}.pem"
    local key_file="${name}-key.pem"
    local csr_file="${name}.csr"

    if [[ -f "$cert_file" && -f "$key_file" && "$FORCE" -eq 0 ]]; then
        skip "$cert_file already exists"
        return
    fi

    info "Issuing cert for '${cn}' (${san})..."

    # Generate private key
    openssl genrsa -out "$key_file" "$KEY_BITS" 2>/dev/null

    # Generate CSR
    openssl req -new \
        -key "$key_file" \
        -out "$csr_file" \
        -subj "/O=RacingPoint/CN=${cn}"

    # Build extension string
    local ext
    if [[ "$is_client" == "yes" ]]; then
        ext="subjectAltName=${san}
basicConstraints=CA:FALSE
keyUsage=critical,digitalSignature
extendedKeyUsage=clientAuth"
    else
        ext="subjectAltName=${san}
basicConstraints=CA:FALSE
keyUsage=critical,digitalSignature,keyEncipherment
extendedKeyUsage=serverAuth,clientAuth"
    fi

    # Sign with CA
    openssl x509 -req \
        -in "$csr_file" \
        -CA ca.pem \
        -CAkey ca-key.pem \
        -CAcreateserial \
        -out "$cert_file" \
        -days "$CERT_DAYS" \
        -extfile <(echo "$ext") \
        2>/dev/null

    rm -f "$csr_file"
    ok "Issued: ${cert_file}"
}

# ── Server cert ───────────────────────────────────────────────────────────────

issue_cert "server" "racecontrol-server" "$SERVER_SANS_EXTENDED" "no"

# ── Bundle (server cert + CA chain) ───────────────────────────────────────────

if [[ -f server.pem && (-z "$(find bundle.pem -newer server.pem 2>/dev/null)" || "$FORCE" -eq 1) ]]; then
    cat server.pem ca.pem > bundle.pem
    ok "Bundle created: bundle.pem"
fi

# ── Per-pod client certs ──────────────────────────────────────────────────────

for i in $(seq 1 "$POD_COUNT"); do
    # Include the pod's known LAN and Tailscale IPs in the SAN
    # These are the static IPs from CLAUDE.md network map
    declare -A POD_LAN_IPS=(
        [1]="192.168.31.89"
        [2]="192.168.31.33"
        [3]="192.168.31.28"
        [4]="192.168.31.88"
        [5]="192.168.31.86"
        [6]="192.168.31.87"
        [7]="192.168.31.38"
        [8]="192.168.31.91"
    )
    declare -A POD_TS_IPS=(
        [1]="100.92.122.89"
        [2]="100.105.93.108"
        [3]="100.69.231.26"
        [4]="100.75.45.10"
        [5]="100.110.133.87"
        [6]="100.127.149.17"
        [7]="100.82.196.28"
        [8]="100.98.67.67"
    )

    local_ip="${POD_LAN_IPS[$i]:-}"
    ts_ip="${POD_TS_IPS[$i]:-}"

    san="DNS:pod-${i}"
    [[ -n "$local_ip" ]] && san="${san},IP:${local_ip}"
    [[ -n "$ts_ip" ]]    && san="${san},IP:${ts_ip}"

    issue_cert "pod-${i}" "pod-${i}" "$san" "yes"
done

# ── POS client cert ───────────────────────────────────────────────────────────

issue_cert "pos" "pos-terminal" "IP:192.168.31.20,IP:100.95.211.1,DNS:pos1" "yes"

# ── Permissions: protect private keys ─────────────────────────────────────────

chmod 600 ./*-key.pem 2>/dev/null || warn "chmod failed (expected on Windows)"

# ── Summary ───────────────────────────────────────────────────────────────────

echo ""
echo "=== RacingPoint Venue CA — Certificate Inventory ==="
echo ""
echo "Output directory: $(pwd)"
echo ""
echo "CA:"
echo "  ca.pem          — distribute to all nodes (trust anchor)"
echo "  ca-key.pem      — KEEP SECRET (server only)"
echo ""
echo "Server:"
echo "  server.pem      — racecontrol :8080 TLS cert"
echo "  server-key.pem  — racecontrol :8080 private key"
echo "  bundle.pem      — server + CA chain (for TLS config)"
echo ""
echo "Pods:"
for i in $(seq 1 "$POD_COUNT"); do
    echo "  pod-${i}.pem / pod-${i}-key.pem"
done
echo "  pos.pem / pos-key.pem"
echo ""
echo "=== Deployment ==="
echo ""
echo "Server (racecontrol.toml):"
echo "  [tls]"
echo "  enabled = true"
echo "  ca_cert_path = \"$(pwd)/ca.pem\""
echo "  server_cert_path = \"$(pwd)/server.pem\""
echo "  server_key_path = \"$(pwd)/server-key.pem\""
echo "  require_client_cert = false  # set true when all pods have client certs"
echo ""
echo "Pod N (rc-agent.toml):"
echo "  [tls]"
echo "  enabled = true"
echo "  ca_cert_path = \"$(pwd)/ca.pem\""
echo "  server_cert_path = \"$(pwd)/pod-N.pem\""
echo "  server_key_path = \"$(pwd)/pod-N-key.pem\""
echo ""
echo "Done."
