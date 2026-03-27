#!/usr/bin/env node

/**
 * juno-cua setup — configures AI coding agents to use Juno's desktop automation.
 *
 * Detects installed agents, checks for the juno-cua binary, and adds MCP config
 * + skill files so agents can screenshot, click, type, and control your desktop.
 *
 * Usage:
 *   npx juno-cua          # Interactive setup
 *   npx juno-cua --yes    # Auto-approve all
 *   npx juno-cua --check  # Just check what's installed, don't modify
 */

import { execSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync, symlinkSync, copyFileSync } from "node:fs";
import { createInterface } from "node:readline";
import { homedir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const HOME = homedir();
const AUTO_YES = process.argv.includes("--yes") || process.argv.includes("-y");
const CHECK_ONLY = process.argv.includes("--check");
const HELP = process.argv.includes("--help") || process.argv.includes("-h");

// ── Agent definitions ────────────────────────────────────────────────────────

const AGENTS = [
  {
    name: "Claude Code",
    id: "claude-code",
    mcpConfig: join(HOME, ".claude", "mcp.json"),
    skillDir: join(HOME, ".claude", "skills"),
    detected: () => commandExists("claude"),
  },
  {
    name: "Cursor",
    id: "cursor",
    mcpConfig: join(HOME, ".cursor", "mcp.json"),
    skillDir: null,
    detected: () => existsSync(join(HOME, ".cursor")),
  },
  {
    name: "VS Code (Copilot)",
    id: "vscode",
    mcpConfig: join(HOME, ".vscode", "mcp.json"),
    skillDir: null,
    detected: () => existsSync(join(HOME, ".vscode")) || commandExists("code"),
  },
  {
    name: "Windsurf",
    id: "windsurf",
    mcpConfig: join(HOME, ".codeium", "windsurf", "mcp_config.json"),
    skillDir: null,
    detected: () => existsSync(join(HOME, ".codeium", "windsurf")),
  },
  {
    name: "Codex",
    id: "codex",
    mcpConfig: join(HOME, ".codex", "mcp.json"),
    skillDir: null,
    detected: () => commandExists("codex"),
  },
  {
    name: "Gemini CLI",
    id: "gemini",
    mcpConfig: join(HOME, ".gemini", "settings.json"),
    skillDir: null,
    detected: () => commandExists("gemini"),
  },
];

const MCP_ENTRY = {
  command: "juno-cua",
  args: ["serve-mcp"],
};

// ── Helpers ──────────────────────────────────────────────────────────────────

function commandExists(cmd) {
  try {
    execSync(`which ${cmd}`, { stdio: "ignore" });
    return true;
  } catch {
    return false;
  }
}

function log(msg) {
  console.log(msg);
}

function success(msg) {
  console.log(`  [ok] ${msg}`);
}

function warn(msg) {
  console.log(`  [!!] ${msg}`);
}

function info(msg) {
  console.log(`  [..] ${msg}`);
}

function skip(msg) {
  console.log(`  [--] ${msg}`);
}

async function confirm(question) {
  if (AUTO_YES) return true;
  const rl = createInterface({ input: process.stdin, output: process.stdout });
  return new Promise((resolve) => {
    rl.question(`  ${question} (y/N) `, (answer) => {
      rl.close();
      resolve(answer.trim().toLowerCase() === "y");
    });
  });
}

function readJson(path) {
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch {
    return null;
  }
}

function writeJson(path, data) {
  const dir = dirname(path);
  if (!existsSync(dir)) mkdirSync(dir, { recursive: true });
  writeFileSync(path, JSON.stringify(data, null, 2) + "\n");
}

// ── Binary check ─────────────────────────────────────────────────────────────

function checkBinary() {
  log("\n--- juno-cua binary ---");

  if (commandExists("juno-cua")) {
    let version = "unknown";
    try {
      version = execSync("juno-cua --version", { encoding: "utf8" }).trim();
    } catch { /* ignore */ }
    success(`Found: ${version}`);
    return true;
  }

  warn("juno-cua not found on PATH");
  log("");
  log("  Install via Homebrew:");
  log("    brew install lacymorrow/tap/juno-cua");
  log("");
  log("  Or build from source:");
  log("    git clone https://github.com/lacymorrow/juno");
  log("    cd juno && cargo build -p juno-cua --release");
  log("    cp target/release/juno-cua /usr/local/bin/");
  log("");
  return false;
}

// ── MCP config injection ─────────────────────────────────────────────────────

function hasMcpEntry(config) {
  const servers = config?.mcpServers || {};
  return "juno" in servers || "juno-cua" in servers;
}

function injectMcpEntry(config) {
  if (!config) config = {};
  if (!config.mcpServers) config.mcpServers = {};
  config.mcpServers["juno"] = MCP_ENTRY;
  return config;
}

async function configureMcp(agent) {
  const existing = readJson(agent.mcpConfig);

  if (existing && hasMcpEntry(existing)) {
    skip(`${agent.name}: MCP already configured`);
    return;
  }

  if (CHECK_ONLY) {
    info(`${agent.name}: MCP not configured (would add)`);
    return;
  }

  const yes = await confirm(`Add juno MCP server to ${agent.name}?`);
  if (!yes) {
    skip(`${agent.name}: skipped`);
    return;
  }

  // For Gemini, the MCP config lives under a different key
  if (agent.id === "gemini") {
    let settings = existing || {};
    if (!settings.mcpServers) settings.mcpServers = {};
    settings.mcpServers["juno"] = MCP_ENTRY;
    writeJson(agent.mcpConfig, settings);
  } else {
    const config = injectMcpEntry(existing);
    writeJson(agent.mcpConfig, config);
  }

  success(`${agent.name}: MCP configured at ${agent.mcpConfig}`);
}

// ── Skill installation (Claude Code only) ────────────────────────────────────

async function installSkill(agent) {
  if (!agent.skillDir) return;

  const skillTarget = join(agent.skillDir, "juno");
  const skillSource = resolve(join(__dirname, "..", "skill"));

  if (existsSync(skillTarget)) {
    skip(`${agent.name}: Skill already installed`);
    return;
  }

  if (CHECK_ONLY) {
    info(`${agent.name}: Skill not installed (would add)`);
    return;
  }

  const yes = await confirm(`Install juno skill for ${agent.name}?`);
  if (!yes) {
    skip(`${agent.name}: skill skipped`);
    return;
  }

  if (!existsSync(agent.skillDir)) {
    mkdirSync(agent.skillDir, { recursive: true });
  }

  // Copy SKILL.md into the skill directory
  mkdirSync(skillTarget, { recursive: true });
  const srcSkill = join(skillSource, "SKILL.md");
  const dstSkill = join(skillTarget, "SKILL.md");

  if (existsSync(srcSkill)) {
    copyFileSync(srcSkill, dstSkill);
    success(`${agent.name}: Skill installed at ${skillTarget}`);
  } else {
    warn(`Skill source not found at ${srcSkill}`);
  }
}

// ── Main ─────────────────────────────────────────────────────────────────────

async function main() {
  if (HELP) {
    log("juno-cua — Setup desktop automation for AI coding agents");
    log("");
    log("Usage:");
    log("  npx juno-cua          Interactive setup");
    log("  npx juno-cua --yes    Auto-approve all changes");
    log("  npx juno-cua --check  Check status without modifying anything");
    log("  npx juno-cua --help   Show this help");
    log("");
    log("What it does:");
    log("  1. Checks for juno-cua binary on PATH");
    log("  2. Detects installed AI coding agents");
    log("  3. Adds MCP server config so agents can use desktop automation");
    log("  4. Installs the juno skill (Claude Code only)");
    process.exit(0);
  }

  log("juno-cua setup — Desktop automation for AI agents");
  log("==================================================");

  // 1. Check binary
  const hasBinary = checkBinary();

  // 2. Detect agents
  log("\n--- Detected agents ---");
  const detected = AGENTS.filter((a) => a.detected());

  if (detected.length === 0) {
    warn("No supported agents detected");
    log("  Supported: Claude Code, Cursor, VS Code, Windsurf, Codex, Gemini CLI");
    process.exit(0);
  }

  for (const agent of detected) {
    success(agent.name);
  }

  const notDetected = AGENTS.filter((a) => !a.detected());
  for (const agent of notDetected) {
    skip(`${agent.name} (not found)`);
  }

  if (!hasBinary && !CHECK_ONLY) {
    warn("juno-cua binary not found — MCP server won't work until installed");
    const proceed = await confirm("Continue with config setup anyway?");
    if (!proceed) {
      log("\nInstall juno-cua first, then re-run: npx juno-cua");
      process.exit(0);
    }
  }

  // 3. Configure MCP for detected agents
  log("\n--- MCP configuration ---");
  for (const agent of detected) {
    await configureMcp(agent);
  }

  // 4. Install skills (Claude Code only)
  const skillAgents = detected.filter((a) => a.skillDir);
  if (skillAgents.length > 0) {
    log("\n--- Skill installation ---");
    for (const agent of skillAgents) {
      await installSkill(agent);
    }
  }

  // 5. Summary
  log("\n--- Done ---");
  if (hasBinary) {
    log("  juno-cua is ready. Your agents can now:");
    log("    - Take screenshots and analyze your screen");
    log("    - Click, type, and scroll in any application");
    log("    - Read accessibility trees for precise element targeting");
    log("    - Open apps and URLs, manage the clipboard");
  } else {
    log("  Config written. Install the binary to activate:");
    log("    brew install lacymorrow/tap/juno-cua");
  }
  log("");
}

main().catch((err) => {
  console.error(`Error: ${err.message}`);
  process.exit(1);
});
