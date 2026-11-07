#!/usr/bin/env bash
# ==============================================================================
# Layer 4 Runtime Test Harness (Unit 04)
# Deploys real Roc WebAssembly agent to local Golem server (v1.2.5 / 1.6),
# exercises multi-turn stateful invocations, tests worker crash/restart durability,
# and verifies read-only bypass integrity.
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

PORT="${GOLEM_ROUTER_PORT:-9881}"
COMPONENT_NAME="counter"
WORKER_NAME="counter-test-worker"
SERVER_LOG="/tmp/golem-runtime-test-server.log"
SERVER_PID=""

# Colors
C_RESET='\033[0m'
C_RED='\033[0;31m'
C_GREEN='\033[0;32m'
C_BLUE='\033[0;34m'
C_CYAN='\033[0;36m'
C_BOLD='\033[1m'

log_info() {
    echo -e "${C_CYAN}${C_BOLD}[TEST-RUNTIME]${C_RESET} $*"
}

log_pass() {
    echo -e "${C_GREEN}${C_BOLD}[PASS]${C_RESET} $*"
}

log_fail() {
    echo -e "${C_RED}${C_BOLD}[FAIL]${C_RESET} $*" >&2
}

cleanup() {
    if [[ -n "${SERVER_PID}" ]]; then
        log_info "Stopping Golem server (PID: ${SERVER_PID})..."
        kill "${SERVER_PID}" 2>/dev/null || true
        wait "${SERVER_PID}" 2>/dev/null || true
    fi
}
trap cleanup EXIT INT TERM

cd "${ROOT_DIR}"

log_info "Verifying counter component build..."
if [[ ! -f "build/counter_agent.wasm" ]]; then
    log_info "Building counter agent..."
    ./tooling/build.sh counter
fi

log_info "Starting clean local Golem server on router port ${PORT}..."
golem server run -A --clean --router-port "${PORT}" > "${SERVER_LOG}" 2>&1 &
SERVER_PID=$!

log_info "Waiting for Golem server HTTP API readiness at http://127.0.0.1:${PORT}/v1/components..."
READY=0
for i in {1..30}; do
    if curl -s -f "http://127.0.0.1:${PORT}/v1/components" >/dev/null 2>&1; then
        READY=1
        break
    fi
    sleep 1
done

if [[ ${READY} -ne 1 ]]; then
    log_fail "Golem server failed to start within 30 seconds. Server log:"
    cat "${SERVER_LOG}" >&2
    exit 1
fi
log_pass "Golem server is ready."

# ------------------------------------------------------------------------------
# Test 1: Checklist #3 - Deploy Component
# ------------------------------------------------------------------------------
log_info "Test 1 (Checklist #3): Deploying ${COMPONENT_NAME} component..."
RESP=$(curl -s -F "name=${COMPONENT_NAME}" -F "component=@build/counter_agent.wasm" "http://127.0.0.1:${PORT}/v1/components")
COMPONENT_ID=$(echo "${RESP}" | jq -r '.versionedComponentId.componentId // empty')

if [[ -z "${COMPONENT_ID}" || "${COMPONENT_ID}" == "null" ]]; then
    log_fail "Failed to upload component. Response: ${RESP}"
    cat "${SERVER_LOG}" >&2
    exit 1
fi
log_pass "Checklist #3 PASSED: Component deployed successfully (ID: ${COMPONENT_ID})"

# Create Worker
log_info "Creating worker: ${WORKER_NAME}..."
CREATE_WORKER_RESP=$(curl -s -X POST "http://127.0.0.1:${PORT}/v1/components/${COMPONENT_ID}/workers" \
    -H "Content-Type: application/json" \
    -d "{\"name\": \"${WORKER_NAME}\", \"args\": [], \"env\": {}}")

# Initialize Worker
log_info "Initializing worker ${WORKER_NAME}..."
INIT_OUT=$(golem -A worker invoke "${COMPONENT_NAME}/${WORKER_NAME}" \
    'golem:agent/guest@2.0.0.{initialize}' \
    '"counter-agent"' 'tuple([unstructured-text(inline({data: "{}"}))])' 'anonymous')

if ! echo "${INIT_OUT}" | grep -q "ok"; then
    log_fail "Worker initialization failed: ${INIT_OUT}"
    exit 1
fi
log_pass "Worker initialized successfully."

# ------------------------------------------------------------------------------
# Test 2: Checklist #5 - First Call (Turn 1: Increment)
# ------------------------------------------------------------------------------
log_info "Test 2 (Checklist #5): First call to worker (increment)..."
TURN1_OUT=$(golem -A worker invoke "${COMPONENT_NAME}/${WORKER_NAME}" \
    'golem:agent/guest@2.0.0.{invoke}' \
    '"handle-message"' 'tuple([unstructured-text(inline({data: "increment"}))])' 'anonymous')

if ! echo "${TURN1_OUT}" | grep -q "Counter incremented to 1"; then
    log_fail "Checklist #5 FAILED: Expected 'Counter incremented to 1', got: ${TURN1_OUT}"
    exit 1
fi
log_pass "Checklist #5 PASSED: First call incremented count to 1"

# ------------------------------------------------------------------------------
# Test 3: Checklist #6 - State Across Calls (Multi-turn Mutations)
# ------------------------------------------------------------------------------
log_info "Test 3 (Checklist #6): Multi-turn state persistence across successive calls..."

TURN2_OUT=$(golem -A worker invoke "${COMPONENT_NAME}/${WORKER_NAME}" \
    'golem:agent/guest@2.0.0.{invoke}' \
    '"handle-message"' 'tuple([unstructured-text(inline({data: "increment"}))])' 'anonymous')
if ! echo "${TURN2_OUT}" | grep -q "Counter incremented to 2"; then
    log_fail "Turn 2 FAILED: Expected 'Counter incremented to 2', got: ${TURN2_OUT}"
    exit 1
fi

TURN3_OUT=$(golem -A worker invoke "${COMPONENT_NAME}/${WORKER_NAME}" \
    'golem:agent/guest@2.0.0.{invoke}' \
    '"handle-message"' 'tuple([unstructured-text(inline({data: "increment"}))])' 'anonymous')
if ! echo "${TURN3_OUT}" | grep -q "Counter incremented to 3"; then
    log_fail "Turn 3 FAILED: Expected 'Counter incremented to 3', got: ${TURN3_OUT}"
    exit 1
fi

TURN4_OUT=$(golem -A worker invoke "${COMPONENT_NAME}/${WORKER_NAME}" \
    'golem:agent/guest@2.0.0.{invoke}' \
    '"handle-message"' 'tuple([unstructured-text(inline({data: "get"}))])' 'anonymous')
if ! echo "${TURN4_OUT}" | grep -q "Current count is 3"; then
    log_fail "Checklist #6 FAILED: Expected 'Current count is 3', got: ${TURN4_OUT}"
    exit 1
fi
log_pass "Checklist #6 PASSED: State preserved across multiple turns (count = 3)"

# ------------------------------------------------------------------------------
# Test 4: Checklist #7 - State Across Worker Crash / Restart
# ------------------------------------------------------------------------------
log_info "Test 4 (Checklist #7): Simulating worker crash and recovery..."
golem -A worker simulate-crash "${COMPONENT_NAME}/${WORKER_NAME}"
sleep 1

POST_CRASH_OUT=$(golem -A worker invoke "${COMPONENT_NAME}/${WORKER_NAME}" \
    'golem:agent/guest@2.0.0.{invoke}' \
    '"handle-message"' 'tuple([unstructured-text(inline({data: "get"}))])' 'anonymous')

if ! echo "${POST_CRASH_OUT}" | grep -q "Current count is 3"; then
    log_fail "Checklist #7 FAILED: Expected 'Current count is 3' after restart, got: ${POST_CRASH_OUT}"
    exit 1
fi
log_pass "Checklist #7 PASSED: Durable state successfully recovered across worker crash (count = 3)"

# ------------------------------------------------------------------------------
# Test 5: Checklist #8 - Read-Only Bypass & State Stability
# ------------------------------------------------------------------------------
log_info "Test 5 (Checklist #8): Verifying read-only bypass..."
for i in {1..3}; do
    RO_OUT=$(golem -A worker invoke "${COMPONENT_NAME}/${WORKER_NAME}" \
        'golem:agent/guest@2.0.0.{invoke}' \
        '"handle-message"' 'tuple([unstructured-text(inline({data: "get"}))])' 'anonymous')
    if ! echo "${RO_OUT}" | grep -q "Current count is 3"; then
        log_fail "Checklist #8 FAILED on read-only iteration ${i}: got: ${RO_OUT}"
        exit 1
    fi
done
log_pass "Checklist #8 PASSED: Read-only bypass verified (repeated reads preserve state without mutation)"

# ------------------------------------------------------------------------------
# Final Report
# ------------------------------------------------------------------------------
echo ""
echo -e "${C_GREEN}${C_BOLD}============================================================${C_RESET}"
echo -e "${C_GREEN}${C_BOLD} Layer 4 Runtime Verification Succeeded!${C_RESET}"
echo -e "${C_GREEN}${C_BOLD} Feature Checklist Items Verified:${C_RESET}"
echo -e "  ${C_GREEN}✔${C_RESET} #3  Deploy component to local Golem server"
echo -e "  ${C_GREEN}✔${C_RESET} #5  First call on new worker increments count to 1"
echo -e "  ${C_GREEN}✔${C_RESET} #6  State preserved across multiple turns (count = 3)"
echo -e "  ${C_GREEN}✔${C_RESET} #7  State recovered durably across worker crash/restart"
echo -e "  ${C_GREEN}✔${C_RESET} #8  Read-only bypass preserves state without corruption"
echo -e "${C_GREEN}${C_BOLD}============================================================${C_RESET}"
exit 0
