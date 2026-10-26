#!/usr/bin/env node
// Checks a `biome` binary built with only JavaScript (`--no-default-features --features lang_js`):
// a configuration written for the full build loads, files of other languages are skipped, and
// `biome version` lists the languages.
//
// Usage: node fork/test-reduced-build.mjs <biome binary>

import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { strict as assert } from "node:assert";

const [binary] = process.argv.slice(2);
if (!binary) {
	console.error("Usage: node fork/test-reduced-build.mjs <biome binary>");
	process.exit(1);
}
const biome = resolve(binary);

const project = mkdtempSync(join(tmpdir(), "biome-reduced-"));
try {
	write("biome.json", {
		plugins: ["./rule.grit"],
		overrides: [{ includes: ["**/*.js"], plugins: ["./rule.grit"] }],
		linter: { rules: { suspicious: { noDuplicateFontNames: "error" } } },
		css: { formatter: { enabled: true } },
		graphql: { linter: { enabled: true } },
	});
	writeFileSync(
		join(project, "rule.grit"),
		'`console.log($message)` where { register_diagnostic(span=$message, message="No logging.") }\n',
	);
	writeFileSync(join(project, "a.js"), "debugger;\n");
	writeFileSync(join(project, "a.css"), "a { font-family: Arial, Arial; }\n");
	writeFileSync(join(project, "q.graphql"), "query { a }\n");

	const lint = run("lint", "--reporter=json", ".");
	const report = JSON.parse(lint.stdout);
	const categories = report.diagnostics.map((diagnostic) => diagnostic.category);
	assert.deepEqual(categories, ["lint/suspicious/noDebugger"], "only a.js is linted");
	assert.ok(!lint.stderr.includes("unknown key"), `the configuration of the full build loads:\n${lint.stderr}`);

	const version = run("version");
	assert.match(version.stdout, /Languages: +JavaScript, JSON/);

	console.log("The reduced build loads the full configuration, skips CSS and GraphQL, and lints JavaScript.");
} finally {
	rmSync(project, { recursive: true, force: true });
}

function run(...args) {
	return spawnSync(biome, args, { cwd: project, encoding: "utf8" });
}

function write(name, json) {
	writeFileSync(join(project, name), `${JSON.stringify(json, null, 2)}\n`);
}
