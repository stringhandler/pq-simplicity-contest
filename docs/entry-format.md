# How to enter

Entries live in one folder per category, for example `entries/shrincs-l-stateful/`. Open
a pull request that adds one directory, `entries/<category>/<entry-name>/`, with three
files:

```
entries/<category>/<entry-name>/
  entry.json        metadata
  verifier.simf     your program: one self-contained SimplicityHL file
  convert.lua       maps a shrincs-cpp signature onto your witnesses
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

You don't need `signet_tx`: once your entry is merged, the contest spends it on the
signet and links that transaction (see [the rules](rules.md#on-signet)). Set it only if
you've spent your entry yourself and want the leaderboard to link your transaction
instead. It's taken on trust: the leaderboard doesn't check that the transaction really
spends your program. You can add it later, in a pull request that changes only
`entry.json`; your entry id stays the same.

## `verifier.simf`

One file, already flattened. If you write it in pieces (upstream uses `mcpp`), commit the
preprocessed output. The first 8 bytes of the file's SHA-256, in hex, are your **entry
id**, shown on the leaderboard. See [the rules](rules.md) for what the program must do.

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
`utf8`. There is no `io`, no `os`, no `require`, and no `load`. The string pattern
functions (`find`, `match`, `gmatch`, `gsub`) are removed too: use `sub`, `format`, `rep`
and `..`. Each call has an instruction and memory limit. A script that errors, exceeds a
limit, or returns a value the program rejects counts as a failed test.

Every file in your entry must be a regular file: an entry with a symbolic link in it is
refused.

The baseline entry,
[`entries/shrincs-l-stateful/baseline-upstream-main`](../entries/shrincs-l-stateful/baseline-upstream-main),
is a complete example.

## Testing locally

From a clone of this repository, build the runner image once. It fetches everything it
needs, at the versions the contest pins:

```sh
docker build -t pq-contest-runner .
```

Then test your entry. It takes a few minutes:

```sh
docker run --rm -v "$PWD:/contest" pq-contest-runner \
  test --entry entries/<category>/<entry-name> --out results
```

The terminal lists any failures. Full results, including every case's witness weight,
are in `results/<category>/<entry-name>.json`.

You don't have to: opening the pull request runs the same test, and its check goes red if
your entry doesn't pass. The run's summary lists the failures, or your score.
