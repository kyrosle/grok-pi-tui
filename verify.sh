#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GROK_ROOT="$ROOT"
PI_ROOT="$ROOT/pi-main"
ADAPTER="$GROK_ROOT/crates/codegen/pi-grok-adapter"
LOG_DIR="$ROOT/verification-logs"
mkdir -p "$LOG_DIR"

if command -v cargo >/dev/null 2>&1; then
  "$ROOT/scripts/setup-shared-cargo-target.sh"
fi

python3 "$ADAPTER/scripts/verify_native_grok.py" \
  --workspace "$GROK_ROOT" \
  --pi-source "$PI_ROOT" \
  --json-out "$LOG_DIR/native-grok-verification.json" \
  | tee "$LOG_DIR/native-grok-verification.log"

python3 "$ADAPTER/scripts/test_native_architecture.py" \
  | tee "$LOG_DIR/architecture-negative.log"

ENDPOINT_ARGS=(--workspace "$ROOT")
DEPENDENCY_ARGS=(--json-out "$LOG_DIR/pi-dependency-profile.json")
VERIFY_BINARY="${PI_VERIFY_BINARY:-$ROOT/target/debug/grok-pi}"
if [[ -f "$VERIFY_BINARY" ]]; then
  ENDPOINT_ARGS+=(--binary "$VERIFY_BINARY")
fi
if [[ "${PI_VERIFY_ENFORCE:-0}" == "1" ]]; then
  ENDPOINT_ARGS+=(--enforce)
  DEPENDENCY_ARGS+=(--enforce)
fi
python3 "$ADAPTER/scripts/scan_product_endpoints.py" \
  "${ENDPOINT_ARGS[@]}" \
  --json-out "$LOG_DIR/product-endpoints.json" \
  | tee "$LOG_DIR/product-endpoints.log"

python3 "$ADAPTER/tests/mock_pi_contract.py" \
  --pi-source "$PI_ROOT" \
  --json-out "$LOG_DIR/mock-pi-contract.json" \
  | tee "$LOG_DIR/mock-pi-contract.log"

python3 "$ADAPTER/scripts/check_rust_syntax.py" \
  --workspace "$GROK_ROOT" \
  --json-out "$LOG_DIR/rust-syntax-verification.json" \
  | tee "$LOG_DIR/rust-syntax-verification.log"

if ! command -v cargo >/dev/null 2>&1; then
  cat > "$LOG_DIR/cargo-status.json" <<JSON
{
  "status": "NOT_RUN",
  "reason": "cargo is not installed in this environment",
  "commands": [
    "./scripts/cargo-shared.sh check -p xai-grok-pager-bin --bin grok-pi --no-default-features --features jemalloc,sandbox-enforce",
    "./scripts/cargo-shared.sh check -p xai-grok-pager-bin --bin xai-grok-pager",
    "python3 crates/codegen/pi-grok-adapter/tests/pi_dependency_profile.py",
    "./scripts/cargo-shared.sh test -p pi-grok-adapter",
    "./scripts/cargo-shared.sh test -p xai-grok-pager --lib external_builtin_filter_accepts_aliases_and_omits_product_commands",
    "./scripts/cargo-shared.sh test -p xai-grok-pager --lib slash_compact_with_context_enqueues_command"
  ]
}
JSON
  echo "Static/protocol/mock/syntax verification passed; Cargo verification NOT RUN." >&2
  exit 2
fi

(
  cd "$GROK_ROOT"
  "$ROOT/scripts/cargo-shared.sh" check -p xai-grok-pager-bin --bin grok-pi \
    --no-default-features --features jemalloc,sandbox-enforce
  "$ROOT/scripts/cargo-shared.sh" check -p xai-grok-pager-bin --bin xai-grok-pager
  python3 "$ADAPTER/tests/pi_dependency_profile.py" "${DEPENDENCY_ARGS[@]}"
  "$ROOT/scripts/cargo-shared.sh" test -p pi-grok-adapter
  "$ROOT/scripts/cargo-shared.sh" test -p xai-grok-pager-bin --bin grok-pi \
    --no-default-features --features jemalloc,sandbox-enforce
  "$ROOT/scripts/cargo-shared.sh" test -p xai-grok-pager --lib external_builtin_filter_accepts_aliases_and_omits_product_commands
  "$ROOT/scripts/cargo-shared.sh" test -p xai-grok-pager --lib slash_compact_with_context_enqueues_command
) 2>&1 | tee "$LOG_DIR/cargo-verification.log"

cat > "$LOG_DIR/cargo-status.json" <<JSON
{
  "status": "PASS"
}
JSON

if [[ "${PI_VERIFY_ENFORCE:-0}" == "1" ]]; then
  echo "Selected verification checks passed with strict removal/endpoint policy."
else
  echo "Selected verification checks passed; pending removals and endpoint baseline are reported, not terminal acceptance."
fi
