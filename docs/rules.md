# Rules and scoring

## Categories

There is one leaderboard per category, and one folder per category under `entries/`. For
now there is one; more will follow as the signer supports them.

| Folder | Byte | Scheme | Signature |
|---|---|---|---|
| [`shrincs-l-stateful`](../entries/shrincs-l-stateful) | `0x01` | SHRINCS-L: n = 16, W = 4, l = 64, S = 140, 207 stateful signatures per key | The stateful path, from shrincs-cpp at `fde3603` |

## What your program must do

- **Language:** SimplicityHL, compiled with the contest's compiler, which is the
  SimplicityHL that [tx-manifest-wallet](https://github.com/stringhandler/txmanifest-wallet)
  pins (0.7.1, Bitcoin jet set). It is compiled without debug symbols.
- **Key:** take the public key as exactly one parameter, `param::PK: u256`, holding
  `PK.seed || PK.root`.
- **Message:** verify the signature over `jet::sig_all_hash()`.
- **Witnesses:** any names and types you like. Your converter script produces them.
- **Soundness:** the program must accept every valid signature by the key in `PK` over
  the transaction, and reject anything else.

## How entries are tested

For each entry, the runner:

1. Compiles your program once for each test key.
2. Signs the test transaction's `sig_all_hash` with:
   - the published test keys in [`keys/`](../keys);
   - a set of random keys generated for that run. Their seeds are published with the
     results, so anyone can reproduce the run.
3. Covers a spread of leaf indexes for each key: q = 1, 2, 3, 10, 100, 206 and 207. 207
   is the special last leaf.
4. Runs your converter on each signature, then satisfies, prunes and pads the program
   exactly as tx-manifest-wallet does for a real spend. It then checks the witness stack
   against the Simplicity signet node's acceptance rules: the cost window and minimal
   padding.

**Every valid signature must verify.** These must all be **rejected**:
- a signature by another key;
- a signature over another message;
- a signature with any single byte changed, tested at several positions in each field.

If any valid signature fails, or any of these is accepted, the entry doesn't rank.

**Passing the tests isn't proof of soundness.** They can't tell a verifier that checks the
whole signature from one that checks less, for example comparing only part of the
recomputed root to `PK.root`. Every `verifier.simf` is read before its pull request is
merged, and one that checks less than the full signature isn't merged. Once merged, an
entry that passes ranks.

## Score

**The score is the weight of the witness stack for a first signature (q = 1), in
weight units. Lower is better.** It includes everything the input carries: your witness
data, the program, the CMR, the control block, and the cost padding your program's
computation needs. On Bitcoin, a program must buy its CPU budget with witness weight, so
compute counts toward the score as much as bytes do.

The table also shows the mean over all tested leaf indexes, the largest stack seen, and
the program's size and cost. Ties go to the earlier entry.

## On signet

You don't need to touch the chain. Once your entry passes, the contest puts it on the
Simplicity signet for you, once:
1. It locks a small coin to your program under a throwaway key.
2. It spends the coin with a SHRINCS signature, encoded by your `convert.lua`.

The leaderboard then links the spend in the explorer. The record of every entry spent
this way is [`onchain.json`](../onchain.json). If you've spent your entry yourself, you
can link that transaction instead, with `signet_tx` in `entry.json`.

## Licence

This repository is MIT-licensed (see [`LICENSE`](../LICENSE)). By submitting an entry, you
agree to license it under the same terms, so anyone can reuse the winning verifiers.
