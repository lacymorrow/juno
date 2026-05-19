# Security Policy

Juno gives an AI agent full control of your Mac — screen, keyboard, mouse, filesystem, and shell. We take security reports seriously.

## Supported versions

Security fixes are applied to the latest released version on [GitHub Releases](https://github.com/lacymorrow/juno/releases). Older versions are not patched — please update before reporting.

## Reporting a vulnerability

**Do not open a public issue.**

Use GitHub's private vulnerability reporting:

1. Go to https://github.com/lacymorrow/juno/security/advisories/new
2. Fill in the details — what you found, how to reproduce it, what impact you believe it has

Or email **security@junebug.ai** with the same information.

You should expect:

- An acknowledgement within **3 business days**.
- A confirmed / not-applicable verdict within **10 business days**.
- A fix or mitigation timeline once confirmed. Critical issues are prioritized.

## What to report

Examples of issues we want to know about:

- Sandboxing or permission-tier bypasses in the agent action system
- Command injection in the shell or `juno-cua` tooling
- Path traversal in the File Agent
- MCP server interactions that escape declared scopes
- Local privilege escalation via the Tauri IPC bridge
- Voice / wake-word pipeline issues that leak audio off-device

## Out of scope

- Issues that require an attacker to already have local code execution as the user.
- Findings against forks or non-canonical builds.
- Reports based solely on automated scanner output without a working reproduction.

## Disclosure

We prefer coordinated disclosure: please give us a reasonable window to ship a fix before publishing details. We will credit reporters in the release notes unless you prefer to remain anonymous.

A historical audit (32 issues from 2026-02-08) is tracked in [SECURITY_AUDIT.md](../SECURITY_AUDIT.md) at the repo root.
