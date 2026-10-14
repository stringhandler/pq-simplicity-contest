# Post-quantum signatures on Bitcoin: smallest verifier

A contest to write the smallest [Simplicity](https://github.com/BlockstreamResearch/SimplicityHL)
program that verifies a post-quantum signature on the Bitcoin Simplicity signet.

The signature scheme is fixed: [SHRINCS](https://github.com/BlockstreamResearch/shrincs-specification),
a hash-based scheme. Signatures come from its reference implementation,
[shrincs-cpp](https://github.com/BlockstreamResearch/shrincs-cpp). What you optimize is the
**verifier**: a SimplicityHL program, and the witness layout it reads. Your score is the
size of the witness a spend carries, which includes your program and any padding its
computation needs.

The baseline is upstream's [shrincs-simplicity-verifier](https://github.com/BlockstreamResearch/shrincs-simplicity-verifier)
(`main`), at about 18.8 KB of witness for a first stateful signature. Upstream's own
optimized rewrites reach about 850 vB. Beat them.

- [Rules and scoring](docs/rules.md)
- [How to enter](docs/entry-format.md)
- [The leaderboard](https://stringhandler.github.io/pq-simplicity-contest/) *(published when the
  first results are)*

## How it works

1. You open a pull request adding `entries/<category>/<your-entry>/`: your program, a small Lua
   script that maps a shrincs-cpp signature onto your witnesses, and a metadata file.
2. The runner compiles your program and runs it against signatures from known public keys
   and from freshly generated random keys. Every one must verify. Signatures it should
   reject (another key's, another message's, tampered) must fail.
3. The leaderboard ranks every entry that passes, by score, with one table per category,
   each linking to its source.

*Planned for later:* an "on signet" badge for entries that are also spent on the
Simplicity signet.

> This is a signet experiment. Taproot outputs with an unspendable internal key can
> still be spent through the key path by someone who can compute discrete logs, so the
> contest measures what a post-quantum spend costs on Bitcoin. It doesn't make coins
> quantum-safe.
