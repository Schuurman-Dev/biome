---
"@biomejs/biome": minor
---

Added the `--baseline`, `--baseline-rule` and `--prune-baseline` options to `biome lint` and `biome check`. They record the existing error-level violations of each file in `biome-baseline.json`, so that a rule can be promoted to `error` without fixing or suppressing every violation first. Biome hides the recorded violations, reports all of them again when a file has more than recorded, and reports entries that no longer occur until they are pruned.

```shell
biome lint --baseline
biome lint --baseline-rule=suspicious/noExplicitAny
biome lint --prune-baseline
```
