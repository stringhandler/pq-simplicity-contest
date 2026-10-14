# How to enter

Entries live in one folder per category, for example `entries/shrincs-l-stateful/`. Each
category folder holds its rules for the qualifying transaction, and a template manifest
for it. Open a pull request that adds one directory, `entries/<category>/<entry-name>/`:

```
entries/<category>/<entry-name>/
  entry.json        metadata
  verifier.simf     your program: one self-contained SimplicityHL file
  convert.lua       maps a shrincs-cpp signature onto your witnesses
  txmanifest.json   optional: your copy of the category's qualifying-transaction manifest
```

Use lowercase letters, digits and `-` in the directory name. To submit a new version,
add a new directory, so earlier results stay reproducible.

## `entry.json`

```json
{
  "name": "split-witness",
  "author": "your GitHub handle",
  "description": "One or two sentences on what you changed.",
  "source": "https://github.com/you/your-work (optional: where the unflattened source lives)",
  "signet_tx": "optional: the txid of a signet transaction that spent a coin locked to your verifier"
}
```

`signet_tx` adds an "on signet" link to the leaderboard, pointing at the transaction on the
[Simplicity signet explorer](https://signet.simplicity-lang.org/explorer). It's optional, and
for now it's taken on trust: the leaderboard doesn't yet check that the transaction really
spends your program. You can add it after your entry is merged, without it being tested
again.

## `verifier.simf`

One file, already flattened. If you write it in pieces (upstream uses `mcpp`), commit the
preprocessed output. The file's SHA-256 is your **entry id**: the first 8 bytes, in hex,
go in your on-chain tag. See [the rules](rules.md) for what the program must do.

## `convert.lua`

Your script defines one function, `convert(sig)`. It returns a table that maps each
witness name your program declares to its value, written in SimplicityHL value syntax.
The runner calls it for every test signature.

```lua
function convert(sig)
  return {
    SIG = "...",          -- every witness::NAME your program reads
    OTHER_ROOT = "0x" .. sig.sl_root,
  }
end
```

`sig` describes one stateful SHRINCS-L signature. All byte strings are lowercase hex
without `0x`.

| Field | Type | What |
|---|---|---|
| `q` | integer | leaf index, 1 to 207 |
| `msg` | hex, 32 B | the message signed: the transaction's `sig_all_hash` |
| `pk` | hex, 32 B | `PK.seed ‖ PK.root`, also your program's `param::PK` |
| `pk_seed`, `pk_root` | hex, 16 B each | the two halves of `pk` |
| `raw` | hex | the whole signature as shrincs-cpp produces it |
| `sl_root` | hex, 16 B | `PK.sl`, the stateless tree's root. `PK.root` is a hash of `PK.sf ‖ PK.sl`, so a stateful verifier needs it. |
| `r` | hex, 32 B | the randomizer R |
| `counter` | hex, 4 B | the WOTS+C counter |
| `chains` | array of 64 hex, 16 B each | the WOTS+C chain values |
| `path` | array of `min(q, 206)` hex, 16 B each | the unbalanced tree's authentication path |

`raw` is `sl_root ‖ r ‖ counter ‖ chains ‖ path`. The other fields are that, already
split.

**The sandbox.** Scripts run in Lua 5.4 with only `string`, `table`, `math` and
`utf8`. There is no `io`, no `os`, no `require`, and no `load`. Each call has an
instruction and memory limit. A script that errors, exceeds a limit, or returns a value
the program rejects counts as a failed test.

The baseline entry,
[`entries/shrincs-l-stateful/baseline-upstream-main`](../entries/shrincs-l-stateful/baseline-upstream-main),
is a complete example.

## Testing locally

Build the runner image once. It needs a checkout of
[tx-manifest-wallet](https://github.com/stringhandler/txmanifest-wallet) next to this
repository:

```sh
docker build --build-context txmanifest=../txmanifest-wallet -t pq-contest-runner .
```

Then test your entry. It takes a few minutes:

```sh
docker run --rm -v "$PWD:/contest" pq-contest-runner \
  test --entry entries/<category>/<entry-name> --out results
```

The terminal lists any failures. Full results, including every case's witness weight,
are in `results/<category>/<entry-name>.json`.
