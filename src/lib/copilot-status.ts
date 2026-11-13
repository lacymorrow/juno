/**
 * GitHub Copilot's status, from `get_copilot_status`. Mirrors the Rust
 * `copilot::CopilotStatus`; the backend decides everything here, this only
 * turns it into words.
 */
export interface CopilotStatus {
  /** A token was found and GitHub confirmed it has Copilot. */
  signed_in: boolean;
  /** Some GitHub token was found, working or not. */
  token_found: boolean;
  login: string | null;
  /** `individual`, `business`, `enterprise`, ... as GitHub reports it. */
  plan: string | null;
  /** Where the token came from, already in words ("from the GitHub CLI"). */
  source: string | null;
  /** The Copilot CLI is signed in as this login, and its sign-in can be
   * imported with one Keychain prompt. */
  copilot_cli_login: string | null;
  /** Why a found token cannot be used. */
  problem: string | null;
}

export interface CopilotStatusText {
  headline: string;
  detail: string;
  /** Offer to import the Copilot CLI's sign-in. */
  canImport: boolean;
}

function planName(plan: string | null): string | null {
  if (!plan) return null;
  return plan.charAt(0).toUpperCase() + plan.slice(1);
}

/** What Settings and setup say about Copilot. */
export function describeCopilotStatus(status: CopilotStatus | null): CopilotStatusText {
  if (!status) {
    return { headline: "Checking GitHub Copilot…", detail: "", canImport: false };
  }
  if (status.signed_in) {
    const who = status.login ? ` as ${status.login}` : "";
    const plan = planName(status.plan);
    return {
      headline: "No API key needed",
      detail: `Juno is using your GitHub Copilot${plan ? ` ${plan}` : ""} plan, signed in${who}${
        status.source ? ` (${status.source})` : ""
      }.`,
      canImport: false,
    };
  }
  if (status.copilot_cli_login) {
    return {
      headline: "GitHub Copilot is not connected yet",
      detail: `The Copilot CLI is signed in as ${status.copilot_cli_login}. Use that sign-in, and macOS will ask once for permission to share it with Juno.`,
      canImport: true,
    };
  }
  if (status.problem) {
    return { headline: "GitHub Copilot is not available", detail: status.problem, canImport: false };
  }
  return {
    headline: "GitHub Copilot is not signed in",
    detail: "Run gh auth login in Terminal, then come back to this window.",
    canImport: false,
  };
}
