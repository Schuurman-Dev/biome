# Fork tools

Tools for building and releasing this fork, and for projects that use its `biome-baseline.json`.
They aren't part of Biome.

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

## Building with only the languages you need

`build.mjs` builds a `biome` binary that contains only the languages you choose:

```shell
node fork/build.mjs --languages js,css,html
```

Names: `js` (JavaScript, TypeScript and JSX), `css`, `html`, `graphql`, `markdown`, `yaml`, `grit`,
or `all`. JSON is always included. Embedded code follows the languages: `<script>` in HTML is
handled when `js` is included and `<style>` when `css` is too, `css`/`styled` and `gql` templates
in JavaScript when `css` and `graphql` are, and Markdown code blocks in every included language.
`--plugins` adds GritQL plugins and `--js-plugins` JavaScript plugins, which embed a JavaScript
engine. `biome upgrade` is never included.

| Build | Size (macOS arm64) |
|---|---|
| `all --plugins --js-plugins` (the release, without `biome upgrade`) | 71 MiB |
| `all --plugins --js-plugins --small` | 49 MiB |
| `js,css,html` | 58 MiB |

`--small` optimizes for size instead of speed: the binary is about a third smaller, `biome check`
uses about a tenth more CPU and `biome format` takes about a quarter longer.

The binary is `fork/build/biome`; `biome version` lists its languages. Use it from a project with
`BIOME_BINARY=<path> npx biome ...`, and from an editor with the extension's `biome.lsp.bin`
setting. `--pack` also writes npm packages for this platform to `fork/dist`, as for a release.

A first build takes 30 to 50 minutes. Cargo keeps every compiled crate in `fork/build/cache`
(about 2.5 GB for each set of options), apart from `target/`, so the next build only recompiles
what changed. `--clean` deletes the cache after the build.

Any `biome.json` loads in any build. Settings for a language or plugins that aren't included are
ignored, and files of those languages are skipped.

## Releasing `@schuurman-dev/biome`

Published privately to GitHub Packages (Schuurman-Dev org) as a wrapper plus one package per
binary. The wrapper depends on each binary under its upstream name
(`"@biomejs/cli-darwin-arm64": "npm:@schuurman-dev/cli-darwin-arm64@…"`), so upstream's
`bin/biome` and the editor extensions work unchanged when a project installs it as:

```json
"@biomejs/biome": "npm:@schuurman-dev/biome@2.6.4-baseline.1"
```

To release, push a tag. `.github/workflows/fork_release.yml` builds darwin-arm64, linux-x64 and
linux-arm64 (glibc) with upstream's release flags, then publishes the binaries and the wrapper:

```shell
git tag v2.6.4-baseline.2 && git push origin v2.6.4-baseline.2
```

Versions can't be republished, so bump `-baseline.N` for every release. A platform without a
binary (Windows, Intel macOS, musl Linux such as Alpine) still installs, since the binary
packages are optional and carry `os`/`cpu`/`libc`; it just can't run `biome`.

`.github/workflows/fork_ci.yml` runs fmt, clippy and the tests of the changed crates on every push
to `feat/lint-baseline`. It also checks that each language feature compiles on its own, and runs
`test-reduced-build.mjs` against a JavaScript-only build.

To build and publish by hand instead:

```shell
# BIOME_VERSION is what `biome --version` prints
RUSTFLAGS='-C strip=symbols -C codegen-units=1' BIOME_VERSION=<version> \
  cargo build -p biome_cli --release
node fork/package-npm.mjs <version> darwin-arm64=target/release/biome
# publish binaries before the wrapper; prerelease versions need --tag
(cd fork/dist/cli-darwin-arm64 && npm publish --tag latest --userconfig <npmrc>)
(cd fork/dist/biome && npm publish --tag latest --userconfig <npmrc>)
```
