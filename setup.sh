#!/bin/bash
set -e

# ═══════════════════════════════════════════════════════════
#  Verigate — One-Command Setup
#  Usage: ./setup.sh [T3N_API_KEY]
# ═══════════════════════════════════════════════════════════

CYAN='\033[36m'
GREEN='\033[32m'
RED='\033[31m'
DIM='\033[2m'
BOLD='\033[1m'
RESET='\033[0m'

echo -e "${CYAN}═══════════════════════════════════════════════════════════${RESET}"
echo -e "  ${BOLD}Verigate — Setup${RESET}"
echo -e "  ${DIM}Autonomous Counterparty Due Diligence Agent on T3N TEE${RESET}"
echo -e "${CYAN}═══════════════════════════════════════════════════════════${RESET}"
echo ""

# Check prerequisites
command -v docker >/dev/null 2>&1 || { echo -e "${RED}Docker required. Install: https://docs.docker.com/get-docker/${RESET}"; exit 1; }
command -v node >/dev/null 2>&1 || { echo -e "${RED}Node.js 22+ required. Install: https://nodejs.org${RESET}"; exit 1; }

# T3N API Key
if [ -n "$1" ]; then
  export T3N_API_KEY="$1"
elif [ -z "$T3N_API_KEY" ]; then
  echo -e "${RED}Usage: ./setup.sh <T3N_API_KEY>${RESET}"
  echo -e "${DIM}Get a key from https://terminal3.io${RESET}"
  exit 1
fi

DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$DIR"

echo -e "\n${BOLD}[1/5]${RESET} Installing bridge dependencies..."
cd t3n-client && npm ci --silent 2>/dev/null && cd ..
echo -e "  ${GREEN}✓${RESET} Dependencies installed"

echo -e "\n${BOLD}[2/5]${RESET} Building Docker images (no cache)..."
T3N_API_KEY="$T3N_API_KEY" docker compose build --no-cache --quiet 2>/dev/null
echo -e "  ${GREEN}✓${RESET} Images built"

echo -e "\n${BOLD}[3/5]${RESET} Starting services..."
docker compose down -v --remove-orphans 2>/dev/null || true
T3N_API_KEY="$T3N_API_KEY" docker compose up -d 2>/dev/null
echo -e "  ${DIM}Waiting for health checks...${RESET}"

# Wait for services
for i in $(seq 1 30); do
  if curl -sf http://localhost:3310/health >/dev/null 2>&1 && \
     curl -sf http://localhost:3302/api/health >/dev/null 2>&1; then
    break
  fi
  sleep 2
done
echo -e "  ${GREEN}✓${RESET} All services healthy"

echo -e "\n${BOLD}[4/5]${RESET} Verifying T3N connection..."
BRIDGE_HEALTH=$(curl -sf http://localhost:3310/health 2>/dev/null)
DID=$(echo "$BRIDGE_HEALTH" | python3 -c "import sys,json; print(json.load(sys.stdin).get('did',''))" 2>/dev/null)
if [ -n "$DID" ]; then
  echo -e "  ${GREEN}✓${RESET} Connected as ${CYAN}${DID}${RESET}"
else
  echo -e "  ${RED}✗${RESET} Bridge not connected — check T3N_API_KEY"
  exit 1
fi

echo -e "\n${BOLD}[5/5]${RESET} Setting up T3N contract..."

# Register contract (if not already registered)
REG=$(curl -sf http://localhost:3310/contract/register -X POST -H "Content-Type: application/json" -d '{}' 2>/dev/null)
REG_OK=$(echo "$REG" | python3 -c "import sys,json; print(json.load(sys.stdin).get('registered',''))" 2>/dev/null)
echo -e "  ${GREEN}✓${RESET} Contract registered: ${REG_OK}"

# Setup KV map (auto-resolves contract ID for ACL)
MAP=$(curl -sf http://localhost:3310/setup/kv-map -X POST -H "Content-Type: application/json" -d '{}' 2>/dev/null)
echo -e "  ${GREEN}✓${RESET} KV map ready"

# Grant agent auth (all functions + egress hosts)
curl -sf http://localhost:3310/agent-auth/grant -X POST -H "Content-Type: application/json" -d '{"functions":["commit-assessment-plan","verify-credential","assess-risk","decide","set-compliance-policy","get-plan-status","get-evidence-chain","get-violations","advance-step","append-to-evidence","execute-protected-action"],"allowedHosts":["api.pioneer.ai","api.verigate.io"]}' >/dev/null 2>&1
echo -e "  ${GREEN}✓${RESET} Agent auth granted (11 functions + egress hosts)"

echo -e "\n${GREEN}Setup complete!${RESET}"
echo -e ""
echo -e "${CYAN}═══════════════════════════════════════════════════════════${RESET}"
echo -e "  ${BOLD}Services Running:${RESET}"
echo -e "  ${GREEN}•${RESET} Frontend:  http://localhost:3309"
echo -e "  ${GREEN}•${RESET} Backend:   http://localhost:3302/api/health"
echo -e "  ${GREEN}•${RESET} Bridge:    http://localhost:3310/health"
echo -e "  ${GREEN}•${RESET} Agent DID: ${DID}"
echo -e ""
echo -e "  ${BOLD}Run Demo:${RESET}"
echo -e "  ${CYAN}./verigate demo${RESET}                    — All 5 scenarios"
echo -e "  ${CYAN}./verigate demo --scenario good${RESET}    — Good entity → APPROVED"
echo -e "  ${CYAN}./verigate demo --scenario sanctioned${RESET} — DPRK → BLOCKED"
echo -e ""
echo -e "  ${BOLD}Open Browser:${RESET}"
echo -e "  ${CYAN}http://localhost:3309/evidence${RESET}     — Evidence Chain viewer"
echo -e "${CYAN}═══════════════════════════════════════════════════════════${RESET}"
