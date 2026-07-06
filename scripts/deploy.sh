#!/usr/bin/env bash
# ============================================================================
# Umbra Protocol — Deploy Script
# ============================================================================
# Builds both Soroban contracts and deploys them to the specified network.
#
# Usage:
#   ./scripts/deploy.sh [network] [--no-build]
#
# Arguments:
#   network    Target network: "local" (default), "testnet", "futurenet"
#   --no-build Skip the cargo build step (use existing WASM files)
#
# Examples:
#   ./scripts/deploy.sh                  # Build + deploy to local
#   ./scripts/deploy.sh testnet           # Build + deploy to testnet
#   ./scripts/deploy.sh testnet --no-build # Deploy existing WASM to testnet
#
# Requires:
#   - Rust + wasm32-unknown-unknown target
#   - Soroban CLI
#   - A funded identity (default identity or $SOROBAN_ACCOUNT)
# ============================================================================

set -euo pipefail

# ---------------------------------------------------------------------------
# Config
# ---------------------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

NETWORK="${1:-local}"
NO_BUILD=false
for arg in "$@"; do
    case "$arg" in
        --no-build) NO_BUILD=true ;;
    esac
done

case "$NETWORK" in
    local)
        RPC_URL="${SOROBAN_RPC_URL:-http://localhost:8000/soroban/rpc}"
        PASSPHRASE="${STELLAR_NETWORK_PASSPHRASE:-Standalone Network ; February 2017}"
        ;;
    testnet)
        RPC_URL="${SOROBAN_RPC_URL:-https://soroban-testnet.stellar.org}"
        PASSPHRASE="${STELLAR_NETWORK_PASSPHRASE:-Test SDF Network ; September 2015}"
        ;;
    futurenet)
        RPC_URL="${SOROBAN_RPC_URL:-https://rpc-futurenet.stellar.org}"
        PASSPHRASE="${STELLAR_NETWORK_PASSPHRASE:-Futurenet Network ; January 2025}"
        ;;
    *)
        echo "Usage: $0 [local|testnet|futurenet] [--no-build]"
        exit 1
        ;;
esac

# Optional: override the source identity (default: soroban's configured default)
SOURCE_IDENTITY="${SOROBAN_ACCOUNT:-default}"

echo "╔══════════════════════════════════════════════════════════════╗"
    echo "║   Umbra Protocol Deploy — ${NETWORK}    ║"
    echo "╚══════════════════════════════════════════════════════════════╝"
    echo ""
    echo "  Network:       $NETWORK"
    echo "  RPC URL:       $RPC_URL"
    echo "  Passphrase:    $PASSPHRASE"
    echo "  Source:        $SOURCE_IDENTITY"
    echo ""

# ---------------------------------------------------------------------------
# Prerequisites
# ---------------------------------------------------------------------------

if ! command -v soroban &>/dev/null; then
    echo "ERROR: Soroban CLI not found. Install it first:"
    echo "  https://soroban.stellar.org/docs/getting-started/setup"
    exit 1
fi

# ---------------------------------------------------------------------------
# Build contracts (unless --no-build)
# ---------------------------------------------------------------------------

WASM_AUDIT="$PROJECT_ROOT/target/wasm32-unknown-unknown/release/umbra_audit.wasm"
WASM_ESCROW="$PROJECT_ROOT/target/wasm32-unknown-unknown/release/umbra_escrow.wasm"

if [ "$NO_BUILD" = false ]; then
    echo "────────────────────────────────────────────────────────────────"
    echo " Step 1: Build contracts (wasm32-unknown-unknown release)"
    echo "────────────────────────────────────────────────────────────────"
    cargo build \
        --target wasm32-unknown-unknown \
        --release \
        --manifest-path "$PROJECT_ROOT/Cargo.toml" \
        2>&1 | sed 's/^/   | /'
    echo "  ✓ Build complete"
    echo ""
fi

if [ ! -f "$WASM_AUDIT" ] || [ ! -f "$WASM_ESCROW" ]; then
    echo "ERROR: Contract WASM files not found."
    echo "  Looking for:"
    echo "    $WASM_AUDIT"
    echo "    $WASM_ESCROW"
    echo "  Run 'cargo build --target wasm32-unknown-unknown --release' or use --no-build"
    exit 1
fi

# ---------------------------------------------------------------------------
# Deploy umbra-audit
# ---------------------------------------------------------------------------

echo "────────────────────────────────────────────────────────────────"
echo " Step 2: Deploy umbra-audit"
echo "────────────────────────────────────────────────────────────────"
echo "  WASM: $WASM_AUDIT"

AUDIT_ID=$(soroban contract deploy \
    --wasm "$WASM_AUDIT" \
    --source "$SOURCE_IDENTITY" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$PASSPHRASE" 2>&1 | tail -1)

echo "  ✓ umbra-audit deployed"
echo "  Contract ID: $AUDIT_ID"
echo ""

# ---------------------------------------------------------------------------
# Deploy umbra-escrow
# ---------------------------------------------------------------------------

echo "────────────────────────────────────────────────────────────────"
echo " Step 3: Deploy umbra-escrow"
echo "────────────────────────────────────────────────────────────────"
echo "  WASM: $WASM_ESCROW"

ESCROW_ID=$(soroban contract deploy \
    --wasm "$WASM_ESCROW" \
    --source "$SOURCE_IDENTITY" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$PASSPHRASE" 2>&1 | tail -1)

echo "  ✓ umbra-escrow deployed"
echo "  Contract ID: $ESCROW_ID"
echo ""

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------

echo "╔══════════════════════════════════════════════════════════════╗"
    echo "║  Deployment Complete                                       ║"
    echo "╚══════════════════════════════════════════════════════════════╝"
    echo ""
    echo "  Network:       $NETWORK"
    echo "  umbra-audit:   $AUDIT_ID"
    echo "  umbra-escrow:  $ESCROW_ID"
    echo ""
    echo "  To initialize the contracts, run:"
    echo ""
    echo "    soroban contract invoke \\"
    echo "      --id $AUDIT_ID \\"
    echo "      --source <regulator-identity> \\"
    echo "      --rpc-url $RPC_URL \\"
    echo "      --network-passphrase '$PASSPHRASE' \\"
    echo "      -- fn init --regulator <ADDRESS> --verifier_key <32_BYTE_HEX>"
    echo ""
    echo "    soroban contract invoke \\"
    echo "      --id $ESCROW_ID \\"
    echo "      --source <admin-identity> \\"
    echo "      --rpc-url $RPC_URL \\"
    echo "      --network-passphrase '$PASSPHRASE' \\"
    echo "      -- fn init --admin <ADDRESS> --arbitrator <ADDRESS> --verifier_key <32_BYTE_HEX>"
