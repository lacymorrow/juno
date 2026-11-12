import { describe, expect, it } from "vitest";
import { describeCopilotStatus, type CopilotStatus } from "../copilot-status";
import { providerUnavailableReason } from "../provider-status";
import type { ProviderInfo } from "@/types/settings";

const none: CopilotStatus = {
  signed_in: false,
  token_found: false,
  login: null,
  plan: null,
  source: null,
  copilot_cli_login: null,
  problem: null,
};

describe("describeCopilotStatus", () => {
  it("names the account, plan and where the sign-in came from", () => {
    const text = describeCopilotStatus({
      ...none,
      signed_in: true,
      token_found: true,
      login: "lacy",
      plan: "enterprise",
      source: "from the GitHub CLI",
    });
    expect(text.headline).toBe("No API key needed");
    expect(text.detail).toBe(
      "Juno is using your GitHub Copilot Enterprise plan, signed in as lacy (from the GitHub CLI).",
    );
    expect(text.canImport).toBe(false);
  });

  it("offers the Copilot CLI sign-in when that is all there is", () => {
    const text = describeCopilotStatus({ ...none, copilot_cli_login: "lacy" });
    expect(text.canImport).toBe(true);
    expect(text.detail).toContain("lacy");
  });

  it("passes the backend's reason through", () => {
    const text = describeCopilotStatus({
      ...none,
      token_found: true,
      problem: "This GitHub account does not have access to GitHub Copilot.",
    });
    expect(text.detail).toBe("This GitHub account does not have access to GitHub Copilot.");
    expect(text.canImport).toBe(false);
  });

  it("says how to sign in when nothing was found", () => {
    expect(describeCopilotStatus(none).detail).toContain("gh auth login");
  });
});

describe("providerUnavailableReason for Copilot", () => {
  it("asks for a GitHub sign-in, not an API key", () => {
    const copilot = {
      id: "copilot",
      is_available: false,
      needs_sign_in: true,
    } as ProviderInfo;
    expect(providerUnavailableReason(copilot)).toBe("Sign in to GitHub Copilot");
  });
});
