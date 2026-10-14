# shrincs-l-stateful

The stateful path of SHRINCS-L: n = 16, W = 4, l = 64, S = 140, with 207 stateful
signatures per key. Signatures come from shrincs-cpp at `fde3603`, through
`shrincs-signer`.

Each entry is a folder here, for example
[`baseline-upstream-main`](baseline-upstream-main). See
[how to enter](../../docs/entry-format.md) and [the rules](../../docs/rules.md).

## On signet

You don't need to spend your entry: once it's merged, the contest locks a coin to it and
spends it with a SHRINCS signature, using your `convert.lua`, and the leaderboard links
the spend. See [the rules](../../docs/rules.md#on-signet).

To spend it yourself, tx-manifest-wallet's
[`examples/pq_shrincs`](https://github.com/stringhandler/txmanifest-wallet/tree/pq-shrincs/examples/pq_shrincs)
walks through it. Then put the spend's txid in your `entry.json` as `signet_tx`.
