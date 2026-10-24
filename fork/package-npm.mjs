#!/usr/bin/env node
// Builds the fork's npm packages from upstream's, ready for `npm publish`:
//
//   @schuurman-dev/biome             upstream's wrapper (bin/biome, schemas, licences)
//   @schuurman-dev/cli-<platform>    one per binary given
//
// The wrapper depends on each binary package under its upstream name, e.g.
// "@biomejs/cli-darwin-arm64": "npm:@schuurman-dev/cli-darwin-arm64@<version>".
// With the wrapper itself installed as
// "@biomejs/biome": "npm:@schuurman-dev/biome@<version>", upstream's bin/biome
// and the Biome editor extensions find the binary exactly where they expect it.
//
// Usage: node fork/package-npm.mjs <version> <platform>=<binary> [...]
//   e.g. node fork/package-npm.mjs 2.6.4-baseline.1 darwin-arm64=target/release/biome

import { chmodSync, copyFileSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const UPSTREAM = resolve(REPO_ROOT, "packages/@biomejs/biome");
const OUT = resolve(REPO_ROOT, "fork/dist");
const SCOPE = "@schuurman-dev";
const REGISTRY = "https://npm.pkg.github.com";
const REPOSITORY = "git+https://github.com/Schuurman-Dev/biome.git";

const [version, ...binaries] = process.argv.slice(2);
if (!version || binaries.length === 0) {
	console.error("Usage: node fork/package-npm.mjs <version> <platform>=<binary> [...]");
	process.exit(1);
}

const upstream = JSON.parse(readFileSync(resolve(UPSTREAM, "package.json"), "utf8"));
rmSync(OUT, { recursive: true, force: true });

const optionalDependencies = {};
for (const binary of binaries) {
	const [platform, source] = binary.split("=");
	const [os, cpu] = platform.split("-");
	const name = `cli-${platform}`;
	const root = resolve(OUT, name);
	mkdirSync(root, { recursive: true });

	const executable = os === "win32" ? "biome.exe" : "biome";
	copyFileSync(resolve(REPO_ROOT, source), resolve(root, executable));
	chmodSync(resolve(root, executable), 0o755);
	for (const licence of ["LICENSE-APACHE", "LICENSE-MIT", "ROME-LICENSE-MIT"]) {
		copyFileSync(resolve(UPSTREAM, licence), resolve(root, licence));
	}
	write(resolve(root, "package.json"), {
		name: `${SCOPE}/${name}`,
		version,
		description: `The ${platform} binary of ${SCOPE}/biome, a fork of Biome.`,
		license: upstream.license,
		repository: { type: "git", url: REPOSITORY },
		engines: upstream.engines,
		os: [os],
		cpu: [cpu],
		libc: os === "linux" ? [platform.endsWith("musl") ? "musl" : "glibc"] : undefined,
		files: [executable, "LICENSE-APACHE", "LICENSE-MIT", "ROME-LICENSE-MIT"],
		publishConfig: { registry: REGISTRY },
	});
	optionalDependencies[`@biomejs/${name}`] = `npm:${SCOPE}/${name}@${version}`;
}

const wrapper = resolve(OUT, "biome");
mkdirSync(resolve(wrapper, "bin"), { recursive: true });
copyFileSync(resolve(UPSTREAM, "bin/biome"), resolve(wrapper, "bin/biome"));
chmodSync(resolve(wrapper, "bin/biome"), 0o755);
const files = upstream.files.filter((file) => file !== "bin/biome" && file !== "README.md");
for (const file of files) copyFileSync(resolve(UPSTREAM, file), resolve(wrapper, file));
writeFileSync(
	resolve(wrapper, "README.md"),
	`# ${SCOPE}/biome

A fork of [Biome](https://github.com/biomejs/biome) ${upstream.version} that adds
\`biome lint --baseline\`: a \`biome-baseline.json\` of known lint violations, like ESLint's bulk
suppressions. Source: https://github.com/Schuurman-Dev/biome (branch \`feat/lint-baseline\`).

Install it under Biome's own name, so editor extensions find it:

\`\`\`json
"@biomejs/biome": "npm:${SCOPE}/biome@${version}"
\`\`\`

Binaries: ${binaries.map((binary) => binary.split("=")[0]).join(", ")}.
`,
);
write(resolve(wrapper, "package.json"), {
	name: `${SCOPE}/biome`,
	version,
	description: `A fork of Biome ${upstream.version} with lint baselines (biome-baseline.json).`,
	bin: upstream.bin,
	license: upstream.license,
	repository: { type: "git", url: REPOSITORY },
	engines: upstream.engines,
	files: ["bin/biome", "README.md", ...files],
	optionalDependencies,
	publishConfig: { registry: REGISTRY },
});

console.log(`Wrote ${SCOPE}/biome and ${binaries.length} binary package(s) at ${version} to ${OUT}`);

function write(path, manifest) {
	writeFileSync(path, `${JSON.stringify(manifest, null, 2)}\n`);
}
