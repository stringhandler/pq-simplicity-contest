# shrincs-l-stateful

The stateful path of SHRINCS-L: n = 16, W = 4, l = 64, S = 140, with 207 stateful
signatures per key. Signatures come from shrincs-cpp at `fde3603`, through
`shrincs-signer`. Category byte: `0x01`.

Each entry is a folder here, for example
[`baseline-upstream-main`](baseline-upstream-main). See
[how to enter](../../docs/entry-format.md) and [the rules](../../docs/rules.md).

## The on-signet transaction (planned, not live yet)

You don't need this to rank: a pull request that passes the tests is enough. This is a
draft of a later addition, where spending your entry on the Simplicity signet will earn
it an **on signet** badge.

[`txmanifest.json`](txmanifest.json) is that transaction, written
as a [tx-manifest-wallet](https://github.com/stringhandler/txmanifest-wallet) manifest.
`Lock` sends coins to your verifier under a SHRINCS key. `Spend` spends them with a
signature, and has three outputs:

| Output | What | The indexer checks |
|---|---|---|
| `watch` | 330 sat to the contest watch address | that it exists: this is how the indexer finds the transaction |
| `tag` | `OP_RETURN "PQC" ‖ 0x00 ‖ 0x01 ‖ ENTRY_ID ‖ PK` | which entry, and the key to compile it with |
| `rest` | everything else, less the fee, to your address | nothing |

Read it to see exactly what's checked. To use it:

1. Copy `txmanifest.json` and the two params files into your entry's folder.
2. In the copy, change only the `Spend` input's `witnesses`: list the witnesses your
   `verifier.simf` reads, each as `{ "type": "simplicityhl", "value": "params.<NAME>" }`.
   Add a matching param for each, and remove `SIG` and `OTHER_ROOT` if you don't use them.
3. Run `tx-manifest-wallet pin txmanifest.json` to record your program's hash.
4. Fill in the params files. `ENTRY_ID` is the first 8 bytes of SHA-256 of your
   `verifier.simf`: `shasum -a 256 verifier.simf | cut -c1-16`.

[`baseline-upstream-main/txmanifest.json`](baseline-upstream-main/txmanifest.json) is a
filled-in example, which passes `tx-manifest-wallet validate --strict`.

Spending takes two runs, because the signature is made outside the wallet. The first,
with `--print-sighash`, prints the hash. Sign it with `shrincs-signer sign`, turn the
output into your witness values with your `convert.lua`, put them in the params, and run
the second. The full walkthrough is in tx-manifest-wallet's
[`examples/pq_shrincs`](https://github.com/stringhandler/txmanifest-wallet/tree/pq-shrincs/examples/pq_shrincs).
