import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

import { cliPlanLine } from "../Onboarding";

describe("the finish screen names the CLI plan, one line", () => {
  it.each([
    ["claude_cli", "Juno is using your Claude subscription."],
    ["codex_cli", "Juno is using your Codex subscription."],
    ["copilot_cli", "Juno is using your Copilot subscription."],
  ])("%s", (id, line) => {
    expect(cliPlanLine(id, true)).toBe(line);
  });

  it("says nothing for Claude until sign-in is confirmed", () => {
    expect(cliPlanLine("claude_cli", false)).toBeNull();
  });

  it.each([null, "anthropic", "openai", ""])("says nothing for %s", (id) => {
    expect(cliPlanLine(id, true)).toBeNull();
  });
});
