#!/bin/bash
# Usage: ./scripts/setup-t3n.sh
# Requires: T3N_API_KEY env var set
# One-command setup for Verigate T3N WASM contract deployment.

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

if [ -z "$T3N_API_KEY" ]; then
  echo "ERROR: T3N_API_KEY env var must be set"
  exit 1
fi

BRIDGE_PORT="${BRIDGE_PORT:-3310}"
BRIDGE_URL="http://localhost:${BRIDGE_PORT}"

# 1. Build contract
echo "Building WASM contract..."
cd "$PROJECT_ROOT/t3n-contract"
cargo build --target wasm32-wasip2 --release
cp target/wasm32-wasip2/release/verigate_verify.wasm "$PROJECT_ROOT/t3n-client/verigate_verify.wasm"
echo "  -> WASM binary copied to t3n-client/"

# 2. Start bridge (if not running)
if curl -s "${BRIDGE_URL}/health" > /dev/null 2>&1; then
  echo "Bridge already running on port ${BRIDGE_PORT}"
else
  echo "Starting bridge..."
  cd "$PROJECT_ROOT/t3n-client"
  T3N_API_KEY="$T3N_API_KEY" node bridge.mjs &
  BRIDGE_PID=$!
  echo "  -> Bridge started (PID: $BRIDGE_PID)"

  # Wait for bridge to be ready
  for i in $(seq 1 10); do
    if curl -s "${BRIDGE_URL}/health" > /dev/null 2>&1; then
      break
    fi
    sleep 1
  done

  if ! curl -s "${BRIDGE_URL}/health" > /dev/null 2>&1; then
    echo "ERROR: Bridge failed to start within 10s"
    kill $BRIDGE_PID 2>/dev/null || true
    exit 1
  fi
fi

# 3. Register contract
echo "Registering contract..."
curl -s -X POST "${BRIDGE_URL}/contract/register" | python3 -m json.tool

# 4. Setup KV map
echo "Setting up KV map..."
curl -s -X POST "${BRIDGE_URL}/setup/kv-map" | python3 -m json.tool

# 5. Grant agent auth
echo "Granting agent auth..."
curl -s -X POST "${BRIDGE_URL}/agent-auth/grant" \
  -H 'Content-Type: application/json' \
  -d '{
    "functions": [
      "set-compliance-policy",
      "commit-assessment-plan",
      "verify-credential",
      "assess-risk",
      "decide",
      "execute-protected-action",
      "get-plan-status",
      "get-evidence-chain",
      "get-violations",
      "advance-step",
      "append-to-evidence"
    ],
    "allowedHosts": [
      "api.pioneer.ai",
      "api.verigate.io"
    ]
  }' | python3 -m json.tool

echo ""
echo "Setup complete!"
echo "  Bridge: ${BRIDGE_URL}"
echo "  Contract: verigate_verify.wasm"
echo "  Functions: 11 exported, all authorized"
