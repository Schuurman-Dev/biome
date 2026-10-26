#!/usr/bin/env node
// Builds a `biome` binary with only the languages you choose.
//
// Usage: node fork/build.mjs --languages <names> [options]
//   --languages <names>  comma-separated: js, css, html, graphql, markdown, yaml, grit, or all.
//                        JSON is always included, since biome.json is JSON.
//   --plugins            include GritQL plugins (the `plugins` key of biome.json)
//   --js-plugins         include JavaScript plugins, which embed a JavaScript engine
//   --small              optimize for size instead of speed: about a third smaller, up to a quarter slower
//   --version <version>  what `biome --version` prints; defaults to the highest v* tag
//   --pack               also write npm packages for this platform to fork/dist
//
// e.g. node fork/build.mjs --languages js,css,html
//
// The embed features follow the languages: HTML handles its scripts when JavaScript is included
// and its styles when CSS is too, JavaScript handles CSS-in-JS and GraphQL templates when those
// languages are included, and Markdown handles code blocks in every included language.

import { execFileSync } from "node:child_process";
import { statSync } from "node:fs";
import { dirname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const BINARY = resolve(REPO_ROOT, "target/release", process.platform === "win32" ? "biome.exe" : "biome");

const LANGUAGES = {
	js: ["lang_js", "module_graph", "type_inference"],
	css: ["lang_css"],
	html: ["lang_html"],
	graphql: ["lang_graphql"],
	markdown: ["lang_md"],
	yaml: ["lang_yaml"],
	grit: ["lang_grit"],
};

const { values } = parseArgs({
	options: {
		languages: { type: "string" },
		plugins: { type: "boolean", default: false },
		"js-plugins": { type: "boolean", default: false },
		small: { type: "boolean", default: false },
		version: { type: "string" },
		pack: { type: "boolean", default: false },
	},
});

if (!values.languages) {
	console.error(
		"Usage: node fork/build.mjs --languages <js,css,html,...|all> [--plugins] [--js-plugins] [--small] [--version <version>] [--pack]",
	);
	process.exit(1);
}

const names =
	values.languages === "all" ? Object.keys(LANGUAGES) : values.languages.split(",").map((name) => name.trim());
const unknown = names.filter((name) => !(name in LANGUAGES));
if (unknown.length > 0) {
	console.error(`Unknown language: ${unknown.join(", ")}. Choose from: ${Object.keys(LANGUAGES).join(", ")}, all.`);
	process.exit(1);
}

const has = (name) => names.includes(name);
const features = new Set(names.flatMap((name) => LANGUAGES[name]));
if (has("html") && has("js")) features.add("html_embeds");
if (has("js") && (has("css") || has("graphql"))) features.add("js_embeds");
if (has("markdown")) features.add("md_embeds");
if (values.plugins || values["js-plugins"]) features.add("plugins");
if (values["js-plugins"]) features.add("js_plugin");

const version = values.version ?? latestTag();
const featureList = [...features].sort().join(",");
console.log(`Building biome ${version} with: json, ${names.join(", ")}`);
console.log(`cargo features: ${featureList}`);
if (values.small) console.log("Optimizing for size.");

execFileSync("cargo", ["build", "-p", "biome_cli", "--release", "--no-default-features", "--features", featureList], {
	cwd: REPO_ROOT,
	stdio: "inherit",
	env: {
		...process.env,
		BIOME_VERSION: version,
		RUSTFLAGS: "-C strip=symbols -C codegen-units=1",
		...(values.small && { CARGO_PROFILE_RELEASE_OPT_LEVEL: "s" }),
	},
});

const size = (statSync(BINARY).size / 1024 / 1024).toFixed(1);
console.log(`\n${relative(process.cwd(), BINARY)} (${size} MiB)`);
console.log(`Use it with BIOME_BINARY=${BINARY}, or the editor setting biome.lsp.bin.`);

if (values.pack) {
	const platform = `${process.platform}-${process.arch}`;
	execFileSync(
		"node",
		[resolve(REPO_ROOT, "fork/package-npm.mjs"), version, `${platform}=${relative(REPO_ROOT, BINARY)}`],
		{
			cwd: REPO_ROOT,
			stdio: "inherit",
		},
	);
}

function latestTag() {
	const [tag] = execFileSync("git", ["tag", "--list", "v*", "--sort=-v:refname"], {
		cwd: REPO_ROOT,
		encoding: "utf8",
	}).split("\n");
	if (!tag) {
		console.error("No v* tag found; pass --version.");
		process.exit(1);
	}
	return tag.replace(/^v/, "");
}
