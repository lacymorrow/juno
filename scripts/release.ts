#!/usr/bin/env bun
/**
 * Release script for Juno
 *
 * Handles the full release pipeline:
 *   1. Version bump (all Cargo.toml + package.json files)
 *   2. Build juno-cua binary (arm64 + x86_64 + universal)
 *   3. Git commit + tag + push  (v* tag triggers release-tauri.yml on GitHub)
 *   4. npm publish (juno-cua package)
 *   5. Homebrew formula update (juno-cua)
 *
 * The Tauri DMG and updater latest.json are built and published by
 * `.github/workflows/release-tauri.yml`, triggered by the v* tag push.
 *
 * The juno-www marketing site fetches the latest release dynamically from
 * the GitHub API (see juno-www/app/api/release/route.ts), so this script
 * no longer waits for CI or syncs juno-www.
 *
 * Usage:
 *   bun run release              # interactive — prompts for bump type
 *   bun run release patch        # patch bump  (0.4.11 → 0.4.12)
 *   bun run release minor        # minor bump  (0.4.11 → 0.5.0)
 *   bun run release major        # major bump  (0.4.11 → 1.0.0)
 *   bun run release 1.0.0        # explicit version
 *   bun run release --cua-only   # only release juno-cua
 */

import * as p from "@clack/prompts";
import pc from "picocolors";
import { execSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const ROOT = resolve(import.meta.dirname, "..");
const HOMEBREW_TAP = resolve(ROOT, "../homebrew-tap");
const HOMEBREW_FORMULA = resolve(HOMEBREW_TAP, "Formula/juno-cua.rb");
const NPM_PACKAGE = resolve(ROOT, "packages/juno-cua");

const CUA_ONLY = process.argv.includes("--cua-only");

// ── Helpers ──────────────────────────────────────────────────────────────────

function run(cmd: string, opts?: { cwd?: string; stdio?: "inherit" | "pipe" }) {
	return execSync(cmd, {
		cwd: opts?.cwd ?? ROOT,
		stdio: opts?.stdio ?? "pipe",
		encoding: "utf-8",
		shell: "/bin/bash",
	});
}

function readJson(path: string) {
	return JSON.parse(readFileSync(path, "utf-8"));
}

function writeJson(path: string, data: Record<string, unknown>) {
	writeFileSync(path, JSON.stringify(data, null, 2) + "\n");
}

function bumpVersion(current: string, type: "patch" | "minor" | "major"): string {
	const [major, minor, patch] = current.split(".").map(Number);
	switch (type) {
		case "major": return `${major + 1}.0.0`;
		case "minor": return `${major}.${minor + 1}.0`;
		case "patch": return `${major}.${minor}.${patch + 1}`;
	}
}

function cancelled(): never {
	p.cancel("Release cancelled.");
	process.exit(0);
}

function errorText(err: unknown): string {
	if (err instanceof Error) {
		if ("stderr" in err && typeof err.stderr === "string" && err.stderr.trim())
			return err.stderr.trim();
		return err.message;
	}
	return String(err);
}

// ── npm publish ──────────────────────────────────────────────────────────────

async function publishNpm(): Promise<boolean> {
	if (!existsSync(NPM_PACKAGE)) {
		p.log.warn(`npm package not found at ${pc.dim(NPM_PACKAGE)}`);
		return false;
	}

	const spinner = p.spinner();
	spinner.start("Publishing juno-cua to npm");
	try {
		run("npm publish --access public", { cwd: NPM_PACKAGE });
		spinner.stop(pc.green("Published juno-cua to npm"));
		return true;
	} catch (err: unknown) {
		spinner.stop(pc.yellow("npm publish failed"));
		p.log.message(pc.dim(errorText(err)));
	}

	// Retry loop
	while (true) {
		const action = await p.select({
			message: "How would you like to proceed?",
			options: [
				{ value: "otp" as const, label: "Enter OTP", hint: "publish with one-time password" },
				{ value: "login" as const, label: "Log in to npm", hint: "run npm login, then retry" },
				{ value: "retry" as const, label: "Retry publish" },
				{ value: "skip" as const, label: "Skip npm publish" },
			],
		});

		if (p.isCancel(action) || action === "skip") {
			p.log.info("Skipping npm publish");
			return false;
		}

		if (action === "login") {
			try {
				run("npm login", { cwd: NPM_PACKAGE, stdio: "inherit" });
				p.log.success("Logged in to npm");
			} catch {
				p.log.error("npm login failed");
			}
			continue;
		}

		let otpFlag = "";
		if (action === "otp") {
			const otp = await p.text({
				message: "npm OTP",
				placeholder: "123456",
				validate: (v) => { if (!v || !/^\d{6}$/.test(v.trim())) return "OTP must be 6 digits"; },
			});
			if (p.isCancel(otp)) continue;
			otpFlag = ` --otp ${otp}`;
		}

		const retrySpinner = p.spinner();
		retrySpinner.start("Publishing to npm");
		try {
			run(`npm publish --access public${otpFlag}`, { cwd: NPM_PACKAGE });
			retrySpinner.stop(pc.green("Published to npm"));
			return true;
		} catch (err: unknown) {
			retrySpinner.stop(pc.red("npm publish failed"));
			p.log.message(pc.dim(errorText(err)));
		}
	}
}

// ── Homebrew ─────────────────────────────────────────────────────────────────

async function publishHomebrew(version: string, arm64Sha: string, x64Sha: string) {
	if (!existsSync(HOMEBREW_FORMULA)) {
		p.log.warn(`Homebrew formula not found at ${pc.dim(HOMEBREW_FORMULA)}`);
		return;
	}

	const doHomebrew = await p.confirm({
		message: "Update Homebrew formula?",
		initialValue: true,
	});
	if (p.isCancel(doHomebrew) || !doHomebrew) {
		p.log.info("Skipping Homebrew");
		return;
	}

	const spinner = p.spinner();
	spinner.start("Updating Homebrew formula");

	try {
		run("git checkout main && git pull --rebase origin main", { cwd: HOMEBREW_TAP });

		let formula = readFileSync(HOMEBREW_FORMULA, "utf-8");

		// Update version
		formula = formula.replace(/version "[^"]*"/, `version "${version}"`);

		// Update download URLs
		formula = formula.replace(
			/releases\/download\/v[^/]*\//g,
			`releases/download/v${version}/`,
		);

		// Update SHA256s (arm64 first, then x64 in file order)
		let shaIndex = 0;
		const shas = [arm64Sha, x64Sha];
		formula = formula.replace(/sha256 "[^"]*"/g, () => {
			const sha = shas[shaIndex] || "";
			shaIndex++;
			return `sha256 "${sha}"`;
		});

		writeFileSync(HOMEBREW_FORMULA, formula);

		run("git add Formula/juno-cua.rb", { cwd: HOMEBREW_TAP });
		run(`git commit -m "juno-cua: update to v${version}"`, { cwd: HOMEBREW_TAP });
		run("git push origin main", { cwd: HOMEBREW_TAP });

		spinner.stop(`Homebrew formula updated to ${pc.green(`v${version}`)}`);
	} catch (err: unknown) {
		spinner.stop(pc.red("Homebrew update failed"));
		p.log.error(errorText(err));
	}
}

// ── Main ─────────────────────────────────────────────────────────────────────

async function main() {
	const args = process.argv.slice(2).filter((a) => !a.startsWith("--"));

	console.clear();
	p.intro(pc.magenta(pc.bold(CUA_ONLY ? "  Juno — juno-cua Release  " : "  Juno — Release  ")));

	// Preflight
	const preflight = p.spinner();
	preflight.start("Running preflight checks");

	const status = run("git status --porcelain").trim();
	if (status) {
		preflight.stop(pc.red("Working tree is not clean"));
		p.log.error("Commit or stash changes first:");
		console.log(pc.dim(status));
		process.exit(1);
	}

	for (const tool of ["gh", "cargo"]) {
		try { run(`which ${tool}`); } catch {
			preflight.stop(pc.red(`Required tool '${tool}' not found`));
			process.exit(1);
		}
	}

	preflight.stop("Preflight OK");

	// Determine version
	const pkg = readJson(`${ROOT}/package.json`);
	const currentVersion: string = pkg.version;

	let newVersion: string;
	const arg = args[0];

	if (arg === "patch" || arg === "minor" || arg === "major") {
		newVersion = bumpVersion(currentVersion, arg);
	} else if (arg && /^\d+\.\d+\.\d+$/.test(arg)) {
		newVersion = arg;
	} else {
		const selected = await p.select({
			message: `Current version: ${pc.cyan(currentVersion)}. Bump type?`,
			options: [
				{ value: "patch" as const, label: "patch", hint: `→ ${bumpVersion(currentVersion, "patch")}` },
				{ value: "minor" as const, label: "minor", hint: `→ ${bumpVersion(currentVersion, "minor")}` },
				{ value: "major" as const, label: "major", hint: `→ ${bumpVersion(currentVersion, "major")}` },
			],
		});
		if (p.isCancel(selected)) cancelled();
		newVersion = bumpVersion(currentVersion, selected);
	}

	const tag = `v${newVersion}`;
	const cuaTag = `cua-v${newVersion}`;

	const proceed = await p.confirm({
		message: `Release ${pc.cyan(currentVersion)} → ${pc.green(newVersion)} (${tag})?`,
	});
	if (p.isCancel(proceed) || !proceed) cancelled();

	// 1. Bump versions (all Cargo.toml + package.json)
	const bumpSpinner = p.spinner();
	bumpSpinner.start("Bumping versions");
	run(`bash scripts/bump-version.sh ${newVersion}`);
	bumpSpinner.stop(`Versions bumped to ${pc.green(newVersion)}`);

	// 2. Tauri app build is handled by GitHub Actions (release-tauri.yml) on v* tag push.
	p.log.info("Tauri build → GitHub Actions will build and publish the DMG after tag push.");

	// 3. Build juno-cua (arm64 + x86_64 + universal)
	const cuaSpinner = p.spinner();
	cuaSpinner.start("Building juno-cua (arm64 + x86_64)");
	try {
		run("cargo build -p juno-cua --release --target aarch64-apple-darwin");
		run("cargo build -p juno-cua --release --target x86_64-apple-darwin");

		// Create universal binary
		run(`lipo -create \
			target/aarch64-apple-darwin/release/juno-cua \
			target/x86_64-apple-darwin/release/juno-cua \
			-output target/juno-cua-universal`);

		cuaSpinner.stop(pc.green("juno-cua built (arm64 + x86_64 + universal)"));
	} catch (err: unknown) {
		cuaSpinner.stop(pc.red("juno-cua build failed"));
		p.log.error(errorText(err));
		const cont = await p.confirm({ message: "Continue without juno-cua binaries?" });
		if (p.isCancel(cont) || !cont) cancelled();
	}

	// 4. Package juno-cua archives + compute SHA256
	const archiveSpinner = p.spinner();
	archiveSpinner.start("Packaging juno-cua archives");

	let arm64Sha = "";
	let x64Sha = "";

	try {
		run("tar czf target/juno-cua-darwin-arm64.tar.gz -C target/aarch64-apple-darwin/release juno-cua");
		run("tar czf target/juno-cua-darwin-x64.tar.gz -C target/x86_64-apple-darwin/release juno-cua");
		run("tar czf target/juno-cua-darwin-universal.tar.gz -C target juno-cua-universal --transform 's/juno-cua-universal/juno-cua/'");

		arm64Sha = run("shasum -a 256 target/juno-cua-darwin-arm64.tar.gz | cut -d' ' -f1").trim();
		x64Sha = run("shasum -a 256 target/juno-cua-darwin-x64.tar.gz | cut -d' ' -f1").trim();

		archiveSpinner.stop(pc.green("Archives packaged"));
	} catch (err: unknown) {
		// tar --transform may not be available on macOS, use alternative
		archiveSpinner.stop(pc.yellow("Retrying archive with macOS-compatible tar"));
		try {
			run("cd target && cp juno-cua-universal juno-cua && tar czf juno-cua-darwin-universal.tar.gz juno-cua && rm juno-cua");
			arm64Sha = run("shasum -a 256 target/juno-cua-darwin-arm64.tar.gz | cut -d' ' -f1").trim();
			x64Sha = run("shasum -a 256 target/juno-cua-darwin-x64.tar.gz | cut -d' ' -f1").trim();
			p.log.success("Archives packaged (macOS tar)");
		} catch (err2: unknown) {
			p.log.error(errorText(err2));
		}
	}

	// 5. Changelog
	const lastTag = run("git describe --tags --abbrev=0 2>/dev/null || echo ''").trim();
	let changelog = "";
	if (lastTag) {
		changelog = run(`git log ${lastTag}..HEAD --pretty=format:"- %s (%h)" --no-merges`).trim();
	}
	if (!changelog) changelog = `- Release ${tag}`;
	p.note(changelog, "Changelog");

	// 6. Commit + tag
	const gitSpinner = p.spinner();
	gitSpinner.start("Committing and tagging");
	run("git add -A");
	run(`git commit -m "release: ${tag}"`);
	run(`git tag ${tag}`);
	if (!CUA_ONLY) run(`git tag ${cuaTag}`);
	gitSpinner.stop(`Tagged ${pc.green(tag)}${CUA_ONLY ? "" : ` + ${pc.green(cuaTag)}`}`);

	// 7. Push
	const pushSpinner = p.spinner();
	pushSpinner.start("Pushing to GitHub");
	run("git push origin HEAD --tags");
	pushSpinner.stop("Pushed to GitHub");

	// 8. GitHub Release for Juno app is created by GitHub Actions (release-tauri.yml).
	// juno-cua release is created by GitHub Actions (release-cua.yml) on cua-v* tag push.
	p.log.info(`GitHub Actions will create the ${pc.green(tag)} release with DMG + latest.json.`);

	// 9. npm publish
	await publishNpm();

	// 10. Homebrew
	if (arm64Sha && x64Sha) {
		await publishHomebrew(newVersion, arm64Sha, x64Sha);
	} else {
		p.log.warn("Skipping Homebrew — no SHA256 hashes available");
	}

	// juno-www marketing site fetches the latest release from the GitHub API
	// at request time (cached 5min), so no sync step is needed here.

	// Done
	p.outro(
		`${pc.green("Done!")} Released ${pc.green(tag)} — juno-cua, npm, Homebrew. Tauri DMG → GitHub Actions. juno-www updates automatically.`,
	);
}

main().catch((err) => {
	p.log.error(err.message ?? err);
	process.exit(1);
});
