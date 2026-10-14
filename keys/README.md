# Known test keys

Seeds for the published test keys, as `shrincs-signer` reads them: 48 bytes of hex,
`SK.seed ‖ SK.prf ‖ PK.seed`. Each is SHA-256 of `"pq-contest known key N"`, followed by
the first 16 bytes of SHA-256 of `"pq-contest known key N (2)"`.

They're public on purpose: every entry is tested against them, so anyone can reproduce
a result. **Never lock real coins to them.**

| Key | PK (`PK.seed ‖ PK.root`) |
|---|---|
| `known-1` | `0xe6be8c8c72f54b61b0dfecb484ce7722012e21f74ee97b449fd4d1536ecca7da` |
| `known-2` | `0x4741b505d968499116b7c38d52646338fb79c859355613d8be23645b7c1a708f` |
| `known-3` | `0xe41082b1d9ae6f723ad7b4817432d3ed8231a5eeb7777187ee5808c1f3fc776e` |
