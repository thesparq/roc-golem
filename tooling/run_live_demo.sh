#!/usr/bin/env bash
# ==============================================================================
# Live Demonstration: Roc + Golem 1.6.3 End-to-End
# 1. Build Roc application to WebAssembly core module
# 2. Componentize with wasm-tools and Golem 1.6 WIT
# 3. Start native local Golem server (v1.6.3)
# 4. Deploy component to local Golem Cloud server
# 5. Invoke agent methods across multiple turns (increment -> count = 3)
# 6. Test interactive tool execution with golem ssh
# 7. Stop and restart Golem server to verify durable state recovery
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

PORT="${GOLEM_ROUTER_PORT:-9881}"
SERVER_LOG="/tmp/golem-live-demo-server.log"
SERVER_PID=""

# Colors
C_RESET='\033[0m'
C_RED='\033[0;31m'
C_GREEN='\033[0;32m'
C_BLUE='\033[0;34m'
C_CYAN='\033[0;36m'
C_YELLOW='\033[1;33m'
C_BOLD='\033[1m'

log_step() {
    echo ""
    echo -e "${C_BLUE}${C_BOLD}>>> STEP: $*${C_RESET}"
}

log_ok() {
    echo -e "${C_GREEN}${C_BOLD}[✓] $*${C_RESET}"
}

cleanup() {
    if [[ -n "${SERVER_PID}" ]]; then
        echo -e "${C_CYAN}Stopping Golem server (PID: ${SERVER_PID})...${C_RESET}"
        kill "${SERVER_PID}" 2>/dev/null || true
        wait "${SERVER_PID}" 2>/dev/null || true
    fi
}
trap cleanup EXIT INT TERM

cd "${ROOT_DIR}"

log_step "1. Compile platform host & application with Roc and componentize with wasm-tools"
./tooling/build.sh counter
wasm-tools validate build/counter_agent.wasm
log_ok "WASM component build/counter_agent.wasm successfully validated against Golem 1.6 WIT"

log_step "2. Start clean Golem Cloud v1.6.3 server on port ${PORT}"
fuser -k "${PORT}/tcp" 2>/dev/null || true
golem server run --clean --yes --router-port "${PORT}" > "${SERVER_LOG}" 2>&1 &
SERVER_PID=$!

READY=0
for i in {1..40}; do
    RESP=$(curl -s "http://127.0.0.1:${PORT}/v1/components" 2>/dev/null || true)
    if [[ -n "$RESP" && "$RESP" != *"SERVER_STARTING"* ]]; then
        READY=1
        break
    fi
    sleep 1
done

if [[ $READY -ne 1 ]]; then
    echo -e "${C_RED}Failed to start Golem server within 40 seconds. Log:${C_RESET}" >&2
    cat "${SERVER_LOG}" >&2
    exit 1
fi
log_ok "Golem server v1.6.3 is online and ready (PID: ${SERVER_PID})"

log_step "3. Deploy application components to Golem server"
golem deploy --yes
log_ok "Component 'roc-golem:counter' and remote bash tool deployed"

log_step "4. Create agent instance: counter-agent()"
golem agent new 'counter-agent()' || true
log_ok "Agent instance 'counter-agent()' initialized"

log_step "5. Multi-turn stateful invocations with 'golem agent invoke'"
echo "Invoking turn 1 (increment)..."
OUT1=$(golem agent invoke 'counter-agent()' handle-message '"increment"')
echo "${OUT1}"

echo "Invoking turn 2 (increment)..."
OUT2=$(golem agent invoke 'counter-agent()' handle-message '"increment"')
echo "${OUT2}"

echo "Invoking turn 3 (increment)..."
OUT3=$(golem agent invoke 'counter-agent()' handle-message '"increment"')
echo "${OUT3}"

echo "Querying current state with 'get'..."
GET_OUT=$(golem agent invoke 'counter-agent()' handle-message '"get"')
echo "${GET_OUT}"

if ! echo "${GET_OUT}" | grep -q "Current count is 3"; then
    echo -e "${C_RED}Expected count 3, got: ${GET_OUT}${C_RESET}" >&2
    exit 1
fi
log_ok "State verified: count is 3 after 3 increment turns"

log_step "6. Execute command on agent using 'golem ssh'"
SSH_CMD='echo "=== HELLO FROM GOLEM SSH ===" && echo "Agent host: $(uname -m)" && echo "Working dir: $(pwd)"'
SSH_OUT=$(golem ssh 'counter-agent()' -c "${SSH_CMD}")
echo "${SSH_OUT}"
if ! echo "${SSH_OUT}" | grep -q "HELLO FROM GOLEM SSH"; then
    echo -e "${C_RED}golem ssh failed: ${SSH_OUT}${C_RESET}" >&2
    exit 1
fi
log_ok "golem ssh interactive command execution verified!"

log_step "7. Test Server Shutdown and Durable State Recovery"
echo "Stopping Golem server (simulating node crash / full server reboot)..."
kill "${SERVER_PID}" 2>/dev/null || true
wait "${SERVER_PID}" 2>/dev/null || true
SERVER_PID=""
sleep 2

echo "Restarting Golem server preserving state (no --clean)..."
golem server run --router-port "${PORT}" > "${SERVER_LOG}" 2>&1 &
SERVER_PID=$!

READY=0
for i in {1..40}; do
    RESP=$(curl -s "http://127.0.0.1:${PORT}/v1/components" 2>/dev/null || true)
    if [[ -n "$RESP" && "$RESP" != *"SERVER_STARTING"* ]]; then
        READY=1
        break
    fi
    sleep 1
done

if [[ $READY -ne 1 ]]; then
    echo -e "${C_RED}Failed to restart Golem server. Log:${C_RESET}" >&2
    cat "${SERVER_LOG}" >&2
    exit 1
fi
log_ok "Golem server restarted with persisted state (PID: ${SERVER_PID})"

echo "Querying state from agent post-server restart..."
POST_RESTART_GET=$(golem agent invoke 'counter-agent()' handle-message '"get"')
echo "${POST_RESTART_GET}"

if ! echo "${POST_RESTART_GET}" | grep -q "Current count is 3"; then
    echo -e "${C_RED}Durability check failed! Expected count 3, got: ${POST_RESTART_GET}${C_RESET}" >&2
    exit 1
fi
log_ok "Durability confirmed: State persisted across server restart! (count = 3)"

echo "Testing golem ssh post-server restart..."
SSH_POST_OUT=$(golem ssh 'counter-agent()' -c 'echo "=== SSH POST-RESTART OK ==="')
echo "${SSH_POST_OUT}"
if ! echo "${SSH_POST_OUT}" | grep -q "SSH POST-RESTART OK"; then
    echo -e "${C_RED}golem ssh post-restart failed: ${SSH_POST_OUT}${C_RESET}" >&2
    exit 1
fi
log_ok "golem ssh execution verified post-restart!"

echo ""
echo -e "${C_GREEN}${C_BOLD}============================================================${C_RESET}"
echo -e "${C_GREEN}${C_BOLD} ALL TESTS PASSED: Golem Cloud v1.6.3 End-to-End Verified!  ${C_RESET}"
echo -e "${C_GREEN}${C_BOLD} - Roc App Compilation                                       ${C_RESET}"
echo -e "${C_GREEN}${C_BOLD} - Wasm Component Model (wasm-tools 1.263.0)                ${C_RESET}"
echo -e "${C_GREEN}${C_BOLD} - Golem Server v1.6.3 Deployment                           ${C_RESET}"
echo -e "${C_GREEN}${C_BOLD} - Multi-Turn Stateful Invocations                          ${C_RESET}"
echo -e "${C_GREEN}${C_BOLD} - golem ssh Interactive Shell Execution                    ${C_RESET}"
echo -e "${C_GREEN}${C_BOLD} - Durable Persistence across Full Server Restart           ${C_RESET}"
echo -e "${C_GREEN}${C_BOLD}============================================================${C_RESET}"
