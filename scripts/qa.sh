#!/usr/bin/env bash

set -euo pipefail

SCRIPT_PATH="${BASH_SOURCE[0]}"

# If invoked via PATH (e.g. `qa.sh`), $0/BASH_SOURCE may not include a directory.
# Resolve to an absolute path so we can reliably locate the repo root.
if [ "${SCRIPT_PATH#*/}" = "${SCRIPT_PATH}" ] && [ "${SCRIPT_PATH#/}" = "${SCRIPT_PATH}" ]; then
  SCRIPT_PATH="$(command -v "${SCRIPT_PATH}" || true)"
fi

if [ -z "${SCRIPT_PATH}" ]; then
  echo "Unable to resolve qa.sh path. Run from the repo root with: bash ./scripts/qa.sh" >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${SCRIPT_PATH}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

# Ensure common install locations are available (needed for solana/avm/anchor installed via installers)
export PATH="$HOME/.local/share/solana/install/active_release/bin:$HOME/.cargo/bin:$HOME/.avm/bin:$PATH"
hash -r || true

LOG_FILE=${QA_LOG_FILE:-"qa-$(date -u +%Y%m%dT%H%M%SZ).log"}
exec > >(tee "${LOG_FILE}") 2>&1

echo "QA log: ${LOG_FILE}"
echo "Repo root: ${REPO_ROOT}"
echo "PATH=$PATH"
echo "which solana: $(command -v solana || echo 'not found')"
echo "which anchor: $(command -v anchor || echo 'not found')"
echo "which avm:    $(command -v avm || echo 'not found')"

echo ""
echo "== Disk usage =="
df -h

echo ""
echo "== Tool versions =="
node -v
yarn -v
rustc --version
cargo --version

echo ""
echo "== JS deps =="
if [ -f yarn.lock ]; then
  if yarn install --frozen-lockfile; then
    echo "yarn install --frozen-lockfile succeeded"
  else
    echo "yarn install --frozen-lockfile failed; retrying with plain yarn install"
    yarn install
  fi
else
  echo "yarn.lock not found; running plain yarn install"
  yarn install
fi

echo ""
echo "== Solana CLI =="
if command -v solana >/dev/null 2>&1; then
  solana --version
else
  echo "solana CLI not found in PATH"
  exit 1
fi

echo ""
echo "== Anchor CLI =="
if ! command -v anchor >/dev/null 2>&1; then
  echo "anchor CLI not found; installing via AVM"
  cargo install --git https://github.com/coral-xyz/anchor avm --locked
  export PATH="$HOME/.cargo/bin:$HOME/.avm/bin:$PATH"
  hash -r || true
  avm install 0.31.1
  avm use 0.31.1
fi

hash -r || true
which anchor || true
anchor --version

echo ""
echo "== Anchor/Solana toolchain cache sanity =="
# The Solana SBF toolchain downloads into ~/.cache/solana/ and can become corrupted
# after interrupted installs (e.g., ENOSPC). The most common symptom is:
#   error: not a directory: '~/.cache/solana/v1.53/platform-tools/rust/bin'
# Clearing that cache is safe; it will be re-downloaded.
if [ -d "$HOME/.cache/solana/v1.53" ] && [ ! -d "$HOME/.cache/solana/v1.53/platform-tools/rust/bin" ]; then
  echo "Detected corrupted Solana platform-tools cache at ~/.cache/solana/v1.53; removing..."
  rm -rf "$HOME/.cache/solana/v1.53"
fi

echo ""
echo "== anchor build =="
anchor build

echo ""
echo "== anchor test (all) =="
anchor test

echo ""
echo "== anchor test (tests/svs-7.ts) =="
anchor test --skip-build -- tests/svs-7.ts

echo ""
echo "== SDK tests =="
yarn workspace @stbr/solana-vault test
yarn workspace @stbr/svs-privacy-sdk test

echo ""
echo "QA complete"
