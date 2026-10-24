# Fork tools

Tools for projects that use this fork's `biome-baseline.json`. They aren't part of Biome.

## `ignores-to-baseline.mjs`

Moves `biome-ignore` lint suppressions into the baseline, so the code no longer carries them.

```shell
# from the project root, with a clean git tree
node ignores-to-baseline.mjs --dry-run          # list what would change
node ignores-to-baseline.mjs                    # convert
node ignores-to-baseline.mjs --keep-reason 'INTENTIONAL|false positive'
```

It uses Biome to decide everything, and checks its own result:

1. Lints the project. It stops if there are unsuppressed errors, since the baseline would absorb
   them along with the comments (`--allow-existing-errors` baselines them too).
2. Removes `lint` suppression comments: `//`, `/* */`, multi-line `/** */`, JSX `{/* */}`,
   `<!-- -->` and `#`, including `biome-ignore-all` and `-start`/`-end` ranges. A comment that also
   suppresses `format`, `assist` or `syntax` keeps those categories.
3. Runs `biome lint --baseline`.
4. Puts back each comment that hid a warning, since the baseline only records errors. For each
   candidate it re-lints the file without it, so Biome's own rules for what a comment covers decide.
5. Records the baseline again and checks the project reports the same errors and warnings as at
   the start.

It doesn't touch comments that suppress nothing (Biome reports them as `suppressions/unused`):
removing one can change what other rules see, such as emptying a block. They're listed so you can
delete them.

Options: `--biome <path>` (default `$BIOME_BINARY`, then `node_modules/.bin/biome`),
`--keep-reason <regex>`, `--allow-existing-errors`, `--dry-run`, and paths to limit the run.
