#!/bin/sh
# Run after pulling a new entry PR. In one step:
#   1. build the signer and the runner, if they need it;
#   2. test the entries that are new or changed since their last test;
#   3. put newly passing entries on the Simplicity signet, using operator/wallet.json (from
#      init.sh) or PQ_SIGNET_WALLET, when there is one;
#   4. rebuild the leaderboard in public/.
#
#   ./check.sh                                       test, spend new entries if there's a wallet, rebuild
#   PQ_SIGNET_WALLET=/path/wallet.json ./check.sh    use a wallet somewhere else
#
# Needs tx-manifest-wallet checked out next to this repository, or at TXMANIFEST_WALLET.
set -eu
cd "$(dirname "$0")"

WALLET_REPO=${TXMANIFEST_WALLET:-../txmanifest-wallet}
SIGNER=$WALLET_REPO/contrib/shrincs-signer/bin/shrincs-signer
RUNNER=runner/target/release/runner

if [ ! -d "$WALLET_REPO/txmanifest_lib" ]; then
  echo "No tx-manifest-wallet at $WALLET_REPO; clone it there, or set TXMANIFEST_WALLET." >&2
  exit 1
fi

echo "== building"
make -s -C "$WALLET_REPO/contrib/shrincs-signer"
cargo build --release --quiet --manifest-path runner/Cargo.toml

echo "== testing new and changed entries"
"$RUNNER" test --new-only --signer "$SIGNER" --out results

WALLET=${PQ_SIGNET_WALLET:-operator/wallet.json}
if [ -f "$WALLET" ]; then
  echo "== putting new passing entries on signet"
  "$RUNNER" spend --wallet "$WALLET" --signer "$SIGNER" --results results
else
  echo "== skipping signet: no wallet at $WALLET (run ./init.sh to make one)"
fi

echo "== rebuilding the leaderboard"
"$RUNNER" site --results results --out public

echo
echo "Done. Open public/index.html to check it, then commit onchain.json (if it changed) and publish public/."
