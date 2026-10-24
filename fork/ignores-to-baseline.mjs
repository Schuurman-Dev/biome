#!/usr/bin/env node
// Converts `biome-ignore` lint suppression comments into baseline entries.
//
// 1. Lints the project and records the starting diagnostics.
// 2. Removes lint suppression comments, keeping `format`, `assist` and `syntax`
//    ones, comments whose reason matches --keep-reason, and stale comments that
//    suppress nothing (Biome already reports those; removing one can change
//    what other rules see, e.g. it empties a block).
// 3. Runs `biome lint --baseline`, so the violations those comments hid are
//    recorded in biome-baseline.json.
// 4. Puts back comments whose lines now show warnings: the baseline never
//    records warnings, so removing them would make the warnings appear.
// 5. Records the baseline again and checks that the project reports exactly
//    the diagnostics it reported at the start.
//
// Usage, from the project root (commit or stash your changes first):
//   node ignores-to-baseline.mjs [--biome <path>] [--keep-reason <regex>]
//                                [--allow-existing-errors] [--dry-run] [paths...]

import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { extname, relative, resolve } from "node:path";

const args = parseArgs(process.argv.slice(2));
const root = process.cwd();
const biome = args.biome ?? process.env.BIOME_BINARY ?? resolve(root, "node_modules/.bin/biome");
const keepReason = args.keepReason ? new RegExp(args.keepReason) : null;

function main() {
	const before = lint();
	if (before.errors.length > 0 && !args.allowExistingErrors) {
		fail(
			`The project has ${before.errors.length} lint error(s) that aren't suppressed. ` +
				"They would be baselined too, and become indistinguishable from the converted comments. " +
				"Fix them first, or pass --allow-existing-errors.",
		);
	}

	const staleLines = new Set(
		before.diagnostics
			.filter((diagnostic) => diagnostic.category === "suppressions/unused")
			.map((diagnostic) => `${diagnostic.path}:${diagnostic.line}`),
	);

	const plans = new Map();
	for (const file of filesWithSuppressions()) {
		const source = readFileSync(file, "utf8");
		const comments = findSuppressionComments(source, file, staleLines);
		if (comments.length > 0) plans.set(file, { source, comments });
	}

	const all = [...plans.values()].flatMap((plan) => plan.comments);
	const byAction = (action) => all.filter((comment) => comment.action === action);
	log(
		`Found ${all.length} lint suppression comment(s) in ${plans.size} file(s): ` +
			`${byAction("remove").length + byAction("rewrite").length} to convert, ` +
			`${byAction("keep").length} kept by --keep-reason, ${byAction("stale").length} stale.`,
	);
	if (args.dryRun) {
		for (const [file, plan] of plans) {
			for (const comment of plan.comments) {
				log(`  ${comment.action.padEnd(7)} ${rel(file)}:${comment.line}  ${firstLine(comment.text)}`);
			}
		}
		return;
	}

	// Strip everything we're converting, then find out which comments hid warnings.
	for (const [file, plan] of plans) {
		writeFileSync(file, applyPlan(plan.source, plan.comments, new Set()));
	}
	baseline();
	const stripped = lint();

	// Which comment suppressed a warning depends on how Biome attaches the comment
	// to the code after it, so ask Biome: put back every candidate in the file, then
	// take each one out again and keep it only if a warning reappears.
	const appeared = newDiagnosticKeys(before.warnings, stripped.warnings);
	const restored = new Set();
	for (const [file, plan] of plans) {
		const path = rel(file);
		const categories = [...appeared]
			.filter((key) => key.startsWith(`${path}\0`))
			.map((key) => key.slice(path.length + 1));
		if (categories.length === 0) continue;

		const needed = new Set(
			plan.comments.filter(
				(comment) =>
					converts(comment) &&
					comment.prefixes.some((prefix) => categories.some((category) => matchesPrefix(category, prefix))),
			),
		);
		const expected = count(before.warnings.filter((key) => key.startsWith(`${path}\0`)));
		for (const candidate of [...needed]) {
			needed.delete(candidate);
			writeFileSync(file, applyPlan(plan.source, plan.comments, needed));
			const warnings = lint([path]).warnings;
			if (newDiagnosticKeys([...expected].flatMap(([key, n]) => Array(n).fill(key)), warnings).size > 0) {
				needed.add(candidate);
			}
		}
		for (const comment of needed) restored.add(comment);
		writeFileSync(file, applyPlan(plan.source, plan.comments, needed));
	}

	// A restored comment can also hide errors that were just recorded, so record again.
	baseline();
	const after = lint();

	// Removing a comment that suppressed nothing would also remove its `suppressions/unused`
	// warning; stale comments are kept, but ranges and `-all` comments can still go stale.
	const counted = (keys) => keys.filter((key) => !key.endsWith("\0suppressions/unused"));
	// With --allow-existing-errors, errors that were already reported are baselined too.
	const errorProblems = args.allowExistingErrors
		? diff("error", before.errors, after.errors).filter((problem) => problem.increased)
		: diff("error", before.errors, after.errors);
	const absorbed = args.allowExistingErrors ? before.errors.length - after.errors.length : 0;
	const problems = [
		...errorProblems,
		...diff("warning", counted(before.warnings), counted(after.warnings)),
	];
	if (problems.length > 0) {
		fail(
			"The project doesn't report the same diagnostics as before the conversion:\n" +
				problems.map((problem) => `  ${problem.text}`).join("\n") +
				"\nThe files were left as they are now; `git diff` shows the changes.",
		);
	}

	const converted = byAction("remove").length + byAction("rewrite").length - restored.size;
	log(`Converted ${converted} comment(s) into biome-baseline.json.`);
	if (restored.size > 0) {
		log(`Kept ${restored.size} comment(s) that hid warnings, which the baseline doesn't record:`);
		for (const comment of restored) log(`  ${rel(comment.file)}:${comment.line}  ${firstLine(comment.text)}`);
	}
	if (byAction("stale").length > 0) {
		log(`Left ${byAction("stale").length} stale comment(s) that suppress nothing; delete them by hand:`);
		for (const comment of byAction("stale")) {
			log(`  ${rel(comment.file)}:${comment.line}  ${firstLine(comment.text)}`);
		}
	}
	if (absorbed > 0) log(`Also baselined ${absorbed} error(s) that were reported before the conversion.`);
	log("The project reports the same diagnostics as before, apart from baselined errors.");
}

// Suppression comments ---------------------------------------------------------------

const COMMENT_PATTERNS = {
	slash: /\/\/[^\n]*|\/\*[\s\S]*?\*\//g,
	hash: /#[^\n]*/g,
	html: /<!--[\s\S]*?-->|\/\/[^\n]*|\/\*[\s\S]*?\*\//g,
};

function commentPattern(file) {
	const extension = extname(file).toLowerCase();
	if ([".graphql", ".gql", ".yaml", ".yml"].includes(extension)) return COMMENT_PATTERNS.hash;
	if ([".html", ".vue", ".svelte", ".astro", ".md"].includes(extension)) return COMMENT_PATTERNS.html;
	return COMMENT_PATTERNS.slash;
}

// `biome-ignore[-all|-start|-end] <category>[(value)] ...: <reason>`
const SUPPRESSION =
	/^biome-ignore(-all|-start|-end)?((?:\s+[\w/-]+(?:\([^)]*\))?)+)\s*:\s*([\s\S]*)$/;
const CATEGORY_LIST = /(biome-ignore(?:-all|-start|-end)?)((?:\s+[\w/-]+(?:\([^)]*\))?)+)(\s*:)/;

function findSuppressionComments(source, file, staleLines) {
	const comments = [];
	const ranges = new Map();
	for (const match of source.matchAll(commentPattern(file))) {
		const text = match[0];
		if (!text.includes("biome-ignore")) continue;
		const body = text
			.replace(/^(\/\/|\/\*+|<!--|#)/, "")
			.replace(/(\*\/|-->)$/, "")
			.split("\n")
			.map((line) => line.replace(/^\s*\*?\s?/, ""))
			.join(" ")
			.trim();
		const parsed = SUPPRESSION.exec(body);
		if (!parsed) continue;

		const kind = parsed[1] ?? "";
		const categories = parsed[2].trim().split(/\s+/);
		const reason = parsed[3].trim();
		const lint = categories.filter((category) => category === "lint" || category.startsWith("lint/"));
		if (lint.length === 0) continue;

		const line = source.slice(0, match.index).split("\n").length;
		const rangeKey = categories.join(" ");
		let action;
		if (kind === "-end") {
			// A range end follows the decision made for its start.
			action = ranges.get(rangeKey) ?? "keep";
		} else if (keepReason?.test(reason)) {
			action = "keep";
		} else if (staleLines.has(`${rel(file)}:${line}`)) {
			action = "stale";
		} else {
			action = lint.length === categories.length ? "remove" : "rewrite";
		}
		if (kind === "-start") ranges.set(rangeKey, action);

		comments.push({
			file,
			start: match.index,
			end: match.index + text.length,
			text,
			line,
			kind,
			action,
			prefixes: lint.map((category) => category.replace(/\(.*$/, "")),
			keepCategories: categories.filter((category) => !lint.includes(category)),
		});
	}
	return comments;
}

const converts = (comment) => comment.action === "remove" || comment.action === "rewrite";

/** Rewrites `source`, dropping or rewriting every converted comment not in `restored`. */
function applyPlan(source, comments, restored) {
	let output = "";
	let cursor = 0;
	for (const comment of comments) {
		if (!converts(comment) || restored.has(comment)) continue;
		let edit;
		if (comment.action === "rewrite") {
			const text = comment.text.replace(
				CATEGORY_LIST,
				(_, keyword, _categories, colon) => `${keyword} ${comment.keepCategories.join(" ")}${colon}`,
			);
			edit = { start: comment.start, end: comment.end, text };
		} else {
			edit = commentRemoval(source, comment.start, comment.end);
		}
		output += source.slice(cursor, edit.start) + edit.text;
		cursor = edit.end;
	}
	return output + source.slice(cursor);
}

/** The edit that removes a comment, its JSX `{...}` wrapper, and its line when nothing else is on it. */
function commentRemoval(source, start, end) {
	const lineStart = source.lastIndexOf("\n", start - 1) + 1;
	let lineEnd = source.indexOf("\n", end);
	if (lineEnd === -1) lineEnd = source.length;

	let before = source.slice(lineStart, start);
	let after = source.slice(end, lineEnd);
	if (/\{\s*$/.test(before) && /^\s*\}/.test(after)) {
		before = before.replace(/\{\s*$/, "");
		after = after.replace(/^\s*\}/, "");
	}

	if (before.trim() === "" && after.trim() === "") {
		const next = lineEnd < source.length ? lineEnd + 1 : lineEnd;
		return { start: lineStart, end: next, text: "" };
	}
	return { start: lineStart, end: lineEnd, text: before.replace(/\s+$/, "") + after };
}

function matchesPrefix(category, prefix) {
	return category === prefix || category.startsWith(`${prefix}/`);
}

function filesWithSuppressions() {
	const paths = args.paths.length > 0 ? args.paths : ["."];
	const result = spawnSync("git", ["grep", "-l", "--untracked", "biome-ignore", "--", ...paths], {
		encoding: "utf8",
	});
	if (result.status !== 0 && result.status !== 1) {
		fail("Couldn't list files with `git grep`; run this inside a git repository.");
	}
	return result.stdout
		.split("\n")
		.filter(Boolean)
		.map((file) => resolve(root, file))
		.filter((file) => !file.endsWith("biome-baseline.json"));
}

// Biome ----------------------------------------------------------------------------

function lint(paths = args.paths) {
	const result = spawnSync(
		biome,
		["lint", "--reporter=json", "--max-diagnostics=none", ...paths],
		{ encoding: "utf8", maxBuffer: 1 << 30 },
	);
	if (result.error) fail(`Couldn't run Biome at ${biome}: ${result.error.message}`);
	let report;
	try {
		report = JSON.parse(result.stdout.slice(result.stdout.indexOf("{")));
	} catch {
		fail(`Couldn't read Biome's JSON report:\n${result.stdout}\n${result.stderr}`);
	}
	const diagnostics = (report.diagnostics ?? []).map((diagnostic) => ({
		category: diagnostic.category ?? "",
		severity: diagnostic.severity,
		path: diagnostic.location?.path ?? "",
		line: diagnostic.location?.start?.line ?? 0,
	}));
	const key = (diagnostic) => `${diagnostic.path}\0${diagnostic.category}`;
	return {
		diagnostics,
		errors: diagnostics.filter((d) => d.severity === "error" || d.severity === "fatal").map(key),
		warnings: diagnostics.filter((d) => d.severity === "warning").map(key),
	};
}

function baseline() {
	const result = spawnSync(biome, ["lint", "--baseline", "--max-diagnostics=0", ...args.paths], {
		encoding: "utf8",
	});
	if (result.error || (!existsSync(resolve(root, "biome-baseline.json")) && result.status !== 0)) {
		fail(`\`biome lint --baseline\` failed:\n${result.stdout}\n${result.stderr}`);
	}
}

function count(keys) {
	const counts = new Map();
	for (const key of keys) counts.set(key, (counts.get(key) ?? 0) + 1);
	return counts;
}

/** Keys that occur more often in `after` than in `before`. */
function newDiagnosticKeys(before, after) {
	const was = count(before);
	const now = count(after);
	return new Set([...now.keys()].filter((key) => (was.get(key) ?? 0) < now.get(key)));
}

function diff(severity, before, after) {
	const was = count(before);
	const now = count(after);
	const problems = [];
	for (const key of new Set([...was.keys(), ...now.keys()])) {
		if ((was.get(key) ?? 0) !== (now.get(key) ?? 0)) {
			const [path, category] = key.split("\0");
			problems.push({
				text: `${path} ${category}: ${was.get(key) ?? 0} → ${now.get(key) ?? 0} ${severity}(s)`,
				increased: (now.get(key) ?? 0) > (was.get(key) ?? 0),
			});
		}
	}
	return problems;
}

// Plumbing -------------------------------------------------------------------------

function parseArgs(argv) {
	const parsed = { paths: [], allowExistingErrors: false, dryRun: false };
	for (let index = 0; index < argv.length; index++) {
		const arg = argv[index];
		if (arg === "--biome") parsed.biome = argv[++index];
		else if (arg === "--keep-reason") parsed.keepReason = argv[++index];
		else if (arg === "--allow-existing-errors") parsed.allowExistingErrors = true;
		else if (arg === "--dry-run") parsed.dryRun = true;
		else if (arg.startsWith("--")) fail(`Unknown option ${arg}`);
		else parsed.paths.push(arg);
	}
	return parsed;
}

function rel(file) {
	return relative(root, file).split("\\").join("/");
}

function firstLine(text) {
	const lines = text.trim().split("\n");
	return lines.length > 1 ? `${lines[0]} …` : lines[0];
}

function log(message) {
	console.log(message);
}

function fail(message) {
	console.error(message);
	process.exit(1);
}

main();
