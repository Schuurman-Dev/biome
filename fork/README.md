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

## Releasing `@schuurman-dev/biome`

Published privately to GitHub Packages (Schuurman-Dev org) as a wrapper plus one package per
binary. The wrapper depends on each binary under its upstream name
(`"@biomejs/cli-darwin-arm64": "npm:@schuurman-dev/cli-darwin-arm64@…"`), so upstream's
`bin/biome` and the editor extensions work unchanged when a project installs it as:

```json
"@biomejs/biome": "npm:@schuurman-dev/biome@2.6.4-baseline.1"
```

```shell
# upstream's release flags; BIOME_VERSION is what `biome --version` prints
RUSTFLAGS='-C strip=symbols -C codegen-units=1' BIOME_VERSION=<version> \
  cargo build -p biome_cli --release
node fork/package-npm.mjs <version> darwin-arm64=target/release/biome
# publish binaries before the wrapper; prerelease versions need --tag
(cd fork/dist/cli-darwin-arm64 && npm publish --tag latest --userconfig <npmrc>)
(cd fork/dist/biome && npm publish --tag latest --userconfig <npmrc>)
```

Versions can't be republished, so bump `-baseline.N` for every release. Only macOS arm64 is built
so far; a platform without a binary still installs (the binary packages are optional and carry
`os`/`cpu`), it just can't run `biome`.
