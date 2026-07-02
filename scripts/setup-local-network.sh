#!/usr/bin/env bash
set -euo pipefail

# Umbra Protocol — Local Soroban/Stellar Test Network Setup
#
# Prerequisites:
#   - Rust toolchain with wasm32-unknown-unknown target
#   - Soroban CLI installed (https://soroban.stellar.org/docs/getting-started/setup)
#   - Docker (optional, for running a local Stellar container)
#
# This script starts a local Stellar test network and deploys the Umbra
# contracts for development and testing.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "=== Umbra Protocol — Local Network Setup ==="

# --------------------------------------------------
# 1. Check prerequisites
# --------------------------------------------------
echo "[1/5] Checking prerequisites..."

if ! command -v soroban &>/dev/null; then
    echo "ERROR: Soroban CLI not found. Install it first:"
    echo "  https://soroban.stellar.org/docs/getting-started/setup"
    exit 1
fi

if ! rustup target list --installed 2>/dev/null | grep -q wasm32-unknown-unknown; then
    echo "Installing wasm32-unknown-unknown target..."
    rustup target add wasm32-unknown-unknown
fi

# --------------------------------------------------
# 2. Build contracts
# --------------------------------------------------
echo "[2/5] Building Umbra contracts (wasm)..."
cargo build --target wasm32-unknown-unknown --release --manifest-path "$PROJECT_ROOT/Cargo.toml"

# --------------------------------------------------
# 3. Start local network (Soroban standalone)
# --------------------------------------------------
echo "[3/5] Starting local Soroban network..."

# Use soroban's built-in standalone network if available, otherwise fall back to Docker.
if soroban network ls 2>/dev/null | grep -q "standalone"; then
    echo "Using existing 'standalone' network config."
else
    echo "Adding 'standalone' network configuration..."
    soroban network add \
        --rpc-url http://localhost:8000/soroban/rpc \
        --network-passphrase "Standalone Network ; February 2017" \
        standalone
fi

# Start the local standalone node in the background.
echo "Starting Stellar standalone node (soroban preview)..."
soroban lab start &
NODE_PID=$!
echo "Node PID: $NODE_PID"
echo "Waiting for node to be ready..."

# Give the node a moment to boot
sleep 5

# --------------------------------------------------
# 4. Create test identities
# --------------------------------------------------
echo "[4/5] Creating test identities..."

soroban keys generate --no-fund --network standalone alice 2>/dev/null || true
soroban keys generate --no-fund --network standalone bob 2>/dev/null || true
soroban keys fund alice --network standalone 2>/dev/null || true
soroban keys fund bob --network standalone 2>/dev/null || true

echo "  alice: $(soroban keys address alice 2>/dev/null || echo 'pending')"
echo "  bob:   $(soroban keys address bob 2>/dev/null || echo 'pending')"

# --------------------------------------------------
# 5. Deploy contracts (optional — uncomment when contracts are ready)
# --------------------------------------------------
echo "[5/5] Deploying Umbra contracts..."
echo "  (Skipped — contract deployment implemented in scripts/deploy.sh)"

echo ""
echo "=== Setup Complete ==="
echo "Local Stellar network is running (PID: $NODE_PID)"
echo "To stop: kill $NODE_PID"
echo ""
echo "Next steps:"
echo "  ./scripts/deploy.sh    # Deploy contracts to the local network"
echo "  cargo test --workspace  # Run unit tests"
