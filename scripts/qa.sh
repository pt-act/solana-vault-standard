#!/bin/bash

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${REPO_ROOT}"

LOG_FILE=${QA_LOG_FILE:-"qa-$(date -u +%Y%m%dT%H%M%SZ).log"}

exec > >(tee "${LOG_FILE}") 2>&1

echo "QA log: ${LOG_FILE}"

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
if command -v anchor >/dev/null 2>&1; then
  anchor --version
else
  echo "anchor CLI not found; installing via AVM"
  export PATH="$HOME/.cargo/bin:$PATH"
  cargo install --git https://github.com/coral-xyz/anchor avm --locked
  avm install 0.31.1
  avm use 0.31.1
  anchor --version
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
