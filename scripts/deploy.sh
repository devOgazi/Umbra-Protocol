#!/usr/bin/env bash
set -euo pipefail

# Umbra Protocol — Contract Deployment Script
#
# Deploys umbra-audit and umbra-escrow to the specified Soroban network.
# Usage: ./scripts/deploy.sh [network]
#   network: "local" (default), "testnet", "futurenet"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

NETWORK="${1:-local}"
case "$NETWORK" in
    local)    RPC_URL="http://localhost:8000/soroban/rpc"; PASSPHRASE="Standalone Network ; February 2017" ;;
    testnet)  RPC_URL="https://soroban-testnet.stellar.org"; PASSPHRASE="Test SDF Network ; September 2015" ;;
    futurenet) RPC_URL="https://rpc-futurenet.stellar.org"; PASSPHRASE="Futurenet Network ; January 2025" ;;
    *) echo "Usage: $0 [local|testnet|futurenet]"; exit 1 ;;
esac

echo "=== Umbra Protocol Deploy — $NETWORK ==="
echo "  RPC URL:    $RPC_URL"
echo "  Passphrase: $PASSPHRASE"

WASM_AUDIT="$PROJECT_ROOT/target/wasm32-unknown-unknown/release/umbra_audit.wasm"
WASM_ESCROW="$PROJECT_ROOT/target/wasm32-unknown-unknown/release/umbra_escrow.wasm"

if [ ! -f "$WASM_AUDIT" ] || [ ! -f "$WASM_ESCROW" ]; then
    echo "ERROR: Contract WASM files not found. Run 'cargo build --target wasm32-unknown-unknown --release' first."
    exit 1
fi

# Deploy umbra-audit
echo ""
echo "Deploying umbra-audit..."
AUDIT_ID=$(soroban contract deploy \
    --wasm "$WASM_AUDIT" \
    --source default \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$PASSPHRASE" 2>/dev/null)
echo "  umbra-audit deployed at: $AUDIT_ID"

# Deploy umbra-escrow
echo ""
echo "Deploying umbra-escrow..."
ESCROW_ID=$(soroban contract deploy \
    --wasm "$WASM_ESCROW" \
    --source default \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$PASSPHRASE" 2>/dev/null)
echo "  umbra-escrow deployed at: $ESCROW_ID"

echo ""
echo "=== Deployment Complete ==="
echo "umbra-audit:  $AUDIT_ID"
echo "umbra-escrow: $ESCROW_ID"
