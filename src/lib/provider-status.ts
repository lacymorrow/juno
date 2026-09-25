import type { ProviderInfo } from "@/types/settings";

/**
 * Why a provider cannot be picked, in the words of the thing the person has to
 * go and do about it.
 *
 * Three lists render this same badge, and they used to each carry their own
 * ternary. They disagreed: one said "setup required", the others "No API key",
 * and all three said "No API key" for the one provider that has never wanted a
 * key. The distinction that matters is what you do next, and there are only
 * three answers: paste a key, install Claude Code, or run `claude login`.
 *
 * Returns null when the provider is ready, so callers can render nothing.
 */
export function providerUnavailableReason(provider: ProviderInfo): string | null {
  if (provider.is_available) return null;
  if (provider.needs_sign_in) return "Sign in to Claude Code";
  if (provider.id === "claude_cli") return "Claude Code not installed";
  return "No API key";
}
