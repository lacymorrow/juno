#!/usr/bin/env bun
/**
 * Release script for Juno
 *
 * Usage:
 *   bun run release          # patch bump (default)
 *   bun run release patch    # patch bump
 *   bun run release minor    # minor bump
 *   bun run release major    # major bump
 *   bun run release 1.2.3    # explicit version
 */

import { $ } from "bun";

const REPO_ROOT = new URL("..", import.meta.url).pathname.replace(/\/$/, "");
const DMG_GLOB = `${REPO_ROOT}/src-tauri/target/universal-apple-darwin/release/bundle/dmg/*.dmg`;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async function exec(cmd: string): Promise<string> {
  const result = await $`sh -c ${cmd}`.quiet().nothrow();
  if (result.exitCode !== 0) {
    throw new Error(`Command failed (${result.exitCode}): ${cmd}\n${result.stderr.toString()}`);
  }
  return result.stdout.toString().trim();
}

async function hasCommand(name: string): Promise<boolean> {
  const result = await $`which ${name}`.quiet().nothrow();
  return result.exitCode === 0;
}

function bumpVersion(current: string, bump: string): string {
  const parts = current.split(".").map(Number);
  if (parts.length !== 3 || parts.some(isNaN)) {
    throw new Error(`Invalid current version: ${current}`);
  }
  switch (bump) {
    case "major":
      return `${parts[0] + 1}.0.0`;
    case "minor":
      return `${parts[0]}.${parts[1] + 1}.0`;
    case "patch":
      return `${parts[0]}.${parts[1]}.${parts[2] + 1}`;
    default:
      throw new Error(`Unknown bump type: ${bump}`);
  }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

async function main() {
  const arg = process.argv[2] || "patch";

  console.log("\n🚀 Juno Release\n");

  // 1. Pre-flight checks
  console.log("🔍 Pre-flight checks...");

  // Clean git tree
  const status = await exec("git status --porcelain");
  if (status) {
    throw new Error("Working tree is not clean. Commit or stash changes first.\n" + status);
  }

  // Required tools
  for (const tool of ["gh", "cargo"]) {
    if (!(await hasCommand(tool))) {
      throw new Error(`Required tool '${tool}' not found. Please install it.`);
    }
  }

  // On main branch
  const branch = await exec("git branch --show-current");
  if (branch !== "main") {
    throw new Error(`Must be on 'main' branch (currently on '${branch}').`);
  }

  console.log("  ✓ Git tree clean, tools available, on main branch\n");

  // 2. Determine version
  const pkgPath = `${REPO_ROOT}/package.json`;
  const pkg = await Bun.file(pkgPath).json();
  const currentVersion: string = pkg.version;

  let newVersion: string;
  if (["patch", "minor", "major"].includes(arg)) {
    newVersion = bumpVersion(currentVersion, arg);
  } else if (/^\d+\.\d+\.\d+$/.test(arg)) {
    newVersion = arg;
  } else {
    throw new Error(`Invalid version argument: ${arg}. Use patch, minor, major, or x.y.z`);
  }

  console.log(`📦 Version: ${currentVersion} → ${newVersion}\n`);

  // 3. Bump version using existing script
  console.log("🔄 Bumping versions...");
  await exec(`bash ${REPO_ROOT}/scripts/bump-version.sh ${newVersion}`);
  console.log("  ✓ Versions bumped\n");

  // 4. Build
  console.log("🔨 Building universal macOS binary (this may take a while)...");
  const buildResult = await $`sh -c ${"cd " + REPO_ROOT + " && bun run tauri build --target universal-apple-darwin"}`.quiet().nothrow();
  if (buildResult.exitCode !== 0) {
    throw new Error(`Build failed:\n${buildResult.stderr.toString()}`);
  }
  console.log("  ✓ Build complete\n");

  // 5. Find DMG artifact
  console.log("📀 Looking for DMG artifact...");
  const glob = new Bun.Glob("*.dmg");
  const dmgDir = `${REPO_ROOT}/src-tauri/target/universal-apple-darwin/release/bundle/dmg`;
  const dmgFiles: string[] = [];
  for await (const file of glob.scan(dmgDir)) {
    dmgFiles.push(`${dmgDir}/${file}`);
  }

  if (dmgFiles.length === 0) {
    throw new Error(`No DMG found in ${dmgDir}`);
  }

  // Use the most recently modified DMG
  const dmgPath = dmgFiles.sort((a, b) => {
    const aFile = Bun.file(a);
    const bFile = Bun.file(b);
    return bFile.lastModified - aFile.lastModified;
  })[0];

  console.log(`  ✓ Found: ${dmgPath.split("/").pop()}\n`);

  // 6. Commit + tag
  console.log("📝 Committing and tagging...");
  await exec(`cd ${REPO_ROOT} && git add -A`);
  await exec(`cd ${REPO_ROOT} && git commit -m "release: v${newVersion}"`);
  await exec(`cd ${REPO_ROOT} && git tag v${newVersion}`);
  console.log(`  ✓ Tagged v${newVersion}\n`);

  // 7. Push
  console.log("⬆️  Pushing to origin...");
  await exec(`cd ${REPO_ROOT} && git push origin HEAD --tags`);
  console.log("  ✓ Pushed\n");

  // 8. GitHub Release
  console.log("🎉 Creating GitHub Release...");
  const releaseUrl = await exec(
    `cd ${REPO_ROOT} && gh release create v${newVersion} --title "v${newVersion}" --generate-notes "${dmgPath}"`
  );
  console.log(`  ✓ Release created\n`);

  // 9. Done
  console.log("━".repeat(50));
  console.log(`\n✅ Juno v${newVersion} released!\n`);
  console.log(`   ${releaseUrl}\n`);
}

main().catch((err) => {
  console.error(`\n❌ Release failed: ${err.message}\n`);
  process.exit(1);
});
