#!/usr/bin/env node
// Checks that Biome compiles with any choice of languages, which the default build never shows.
//
// Usage: node fork/check-features.mjs <single|pairs> [--partition M/N] [--dry-run]
//   single  clippy, warnings denied, on biome_service, biome_lsp and biome_cli with each feature
//           on its own (and none)
//   pairs   cargo check on biome_cli with every two features together. The CLI passes its
//           features down to biome_lsp and biome_service, so this covers all three crates.
//   --partition M/N  run only the Mth of N shares of the combinations, for parallel CI jobs
//   --dry-run        list the cargo commands without running them (with --partition, the list
//                    runs past the share; only a real run is split exactly)
//
// Every feature is included except those that don't select languages, so a new language feature
// is checked without editing this script. It needs cargo-hack. Each combination compiles its own
// copy of the crates, so a local run takes a lot of disk; it's meant for CI.

import { execFileSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");

// `stable` is every language at once, which the regular build and tests already cover.
const NOT_LANGUAGES = ["docgen", "schema", "stable", "testing", "unstable"];

const { values, positionals } = parseArgs({
	allowPositionals: true,
	options: {
		partition: { type: "string" },
		"dry-run": { type: "boolean", default: false },
	},
});

const [mode] = positionals;
const runs = {
	single: ["biome_service", "biome_lsp", "biome_cli"].map((crate) => [
		"clippy",
		"-p",
		crate,
		"--each-feature",
		...common(),
		"--",
		"-D",
		"warnings",
	]),
	pairs: [["check", "-p", "biome_cli", "--feature-powerset", "--depth", "2", ...common()]],
}[mode];

if (!runs) {
	console.error("Usage: node fork/check-features.mjs <single|pairs> [--partition M/N] [--dry-run]");
	process.exit(1);
}

for (const args of runs) {
	console.log(`cargo hack ${args.join(" ")}`);
	execFileSync("cargo", ["hack", ...args], {
		cwd: REPO_ROOT,
		stdio: "inherit",
		env: { ...process.env, CARGO_INCREMENTAL: "0" },
	});
}

function common() {
	return [
		"--exclude-features",
		NOT_LANGUAGES.join(","),
		"--keep-going",
		...(process.env.GITHUB_ACTIONS ? ["--log-group", "github-actions"] : []),
		...(values.partition ? ["--partition", values.partition] : []),
		...(values["dry-run"] ? ["--print-command-list"] : []),
	];
}
