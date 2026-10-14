#!/bin/sh
# One-time setup for running the contest:
#   1. build tx-manifest-wallet, the SHRINCS signer and the runner;
#   2. create a bitcoin-signet wallet in operator/ (gitignored), to pay for putting entries
#      on signet;
#   3. ask the Simplicity signet faucet to fund it (one claim per IP per day).
#
#   ./init.sh                                    wallet at operator/wallet.json
#   PQ_SIGNET_WALLET=/path/wallet.json ./init.sh  somewhere else
#   PQ_SKIP_FAUCET=1 ./init.sh                    set up without claiming
#
# Safe to run again: an existing wallet is kept, and only the faucet is asked again.
# Needs tx-manifest-wallet checked out next to this repository, or at TXMANIFEST_WALLET.
set -eu
cd "$(dirname "$0")"

WALLET_REPO=${TXMANIFEST_WALLET:-../txmanifest-wallet}
WALLET=${PQ_SIGNET_WALLET:-operator/wallet.json}
FAUCET=https://signet.simplicity-lang.org/claim
TXW=$WALLET_REPO/target/release/tx-manifest-wallet

if [ ! -d "$WALLET_REPO/txmanifest_lib" ]; then
  echo "No tx-manifest-wallet at $WALLET_REPO; clone it there, or set TXMANIFEST_WALLET." >&2
  exit 1
fi
# The signer should be the one CI scores with: from the commit runner/Cargo.toml pins.
REV=$(sed -n 's/^tx-manifest-lib = .*rev = "\([0-9a-f]\{40\}\)".*/\1/p' runner/Cargo.toml)
if ! git -C "$WALLET_REPO" diff --quiet "$REV" -- contrib/shrincs-signer 2>/dev/null; then
  echo "warning: the signer in $WALLET_REPO differs from $REV, the commit runner/Cargo.toml" >&2
  echo "pins, so scores may not match CI's. To match: git -C $WALLET_REPO checkout $REV" >&2
fi

echo "== building (the first time takes a few minutes)"
make -s -C "$WALLET_REPO/contrib/shrincs-signer"
cargo build --release --quiet --manifest-path "$WALLET_REPO/Cargo.toml" -p tx-manifest-wallet
cargo build --release --quiet --manifest-path runner/Cargo.toml

echo "== signet wallet"
DIR=$(dirname "$WALLET")
mkdir -p "$DIR"
# tx-manifest-wallet reads a config.json beside the wallet file: this one points it at the
# Simplicity signet's Esplora.
[ -f "$DIR/config.json" ] || cp "$WALLET_REPO/examples/bitcoin_covenant/config.json" "$DIR/config.json"
if [ -f "$WALLET" ]; then
  echo "using the existing wallet $WALLET"
else
  # The mnemonic goes into the wallet file; don't print it.
  "$TXW" create-wallet --out "$WALLET" > /dev/null
  chmod 600 "$WALLET"
  echo "created $WALLET (its mnemonic is in the file: keep it private; operator/ is gitignored)"
fi
ADDRESS=$("$TXW" info --wallet "$WALLET" 2>/dev/null | awk '/Receive Address/ { getline; print $1; exit }')
if [ -z "$ADDRESS" ]; then
  echo "could not read the wallet's receive address" >&2
  exit 1
fi
echo "receive address: $ADDRESS"

if [ -n "${PQ_SKIP_FAUCET:-}" ]; then
  echo "== skipping the faucet"
else
  echo "== asking the faucet"
  RESPONSE=$(curl -s --max-time 60 -X POST -H 'Content-Type: application/json' \
    -d "{\"address\":\"$ADDRESS\"}" -w '\n%{http_code}' "$FAUCET" || true)
  CODE=$(printf '%s\n' "$RESPONSE" | tail -n 1)
  BODY=$(printf '%s\n' "$RESPONSE" | sed '$d')
  if [ "$CODE" = "200" ]; then
    echo "funded: $BODY"
  else
    echo "the faucet said ($CODE): $BODY"
    echo "It allows one claim per IP per day. You can also fund $ADDRESS by hand at"
    echo "https://signet.simplicity-lang.org/faucet"
  fi
fi

echo
echo "Ready. After pulling an entry PR, run ./check.sh to test it and put it on signet."
